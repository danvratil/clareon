// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service layer providing a bridge between clareon-core and the Qt UI

mod command;
mod response;
mod worker;

pub use command::Command;
pub use response::{ArtifactData, ErrorCategory, ErrorInfo, MessageData, ModelInfoData, Response};
pub use worker::ServiceWorker;

use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use clareon_core::{
    Config, ConfigManager, ConversationManager, Error, McpManager, Result, Storage,
    tools::{
        ArtifactManager, BubblewrapSandbox, NoneSandbox, SandboxMode, ToolExecutor, ToolRegistry,
        WorkspaceManager, register_builtin_tools,
    },
};

/// Build tool executor (builtins + MCP tools) and return MCP manager.
async fn build_tools(
    config: &Config,
    storage: Arc<Storage>,
) -> Result<(Option<Arc<ToolExecutor>>, Arc<McpManager>)> {
    let mcp_manager = Arc::new(McpManager::new(config.tools.default_timeout));
    if config.mcp.enabled {
        mcp_manager.reload(&config.mcp).await;
    }

    let need_executor = config.tools.enabled || config.mcp.enabled;
    if !need_executor {
        return Ok((None, mcp_manager));
    }

    let mut registry = ToolRegistry::default();
    if config.tools.enabled {
        register_builtin_tools(&mut registry);
    }
    if config.mcp.enabled {
        mcp_manager.register_tools(&mut registry).await;
    }
    let registry = Arc::new(registry);

    use clareon_core::config::SandboxModeConfig;
    let sandbox: Arc<dyn clareon_core::tools::Sandbox> = match config.tools.sandbox_mode {
        SandboxModeConfig::Strict => Arc::new(BubblewrapSandbox::new(SandboxMode::Strict)),
        SandboxModeConfig::Basic => Arc::new(BubblewrapSandbox::new(SandboxMode::Basic)),
        SandboxModeConfig::None => Arc::new(NoneSandbox),
    };

    let workspace_dir = Config::workspace_cache_dir().map_err(std::io::Error::other)?;
    let workspace_manager = Arc::new(WorkspaceManager::new(workspace_dir, Arc::clone(&storage)));
    let artifact_manager = Arc::new(ArtifactManager::new(Arc::clone(&storage)));
    let executor = ToolExecutor::new(registry, sandbox, workspace_manager, artifact_manager);

    Ok((Some(Arc::new(executor)), mcp_manager))
}

/// Handle for sending commands to the service and receiving responses
#[derive(Clone)]
pub struct ServiceHandle {
    command_tx: broadcast::Sender<Command>,
    response_tx: broadcast::Sender<Response>,
}

impl ServiceHandle {
    /// Send a command to the service
    pub fn send(
        &self,
        command: Command,
    ) -> std::result::Result<usize, tokio::sync::broadcast::error::SendError<Command>> {
        self.command_tx.send(command)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Response> {
        self.response_tx.subscribe()
    }
}

/// The main service that owns the runtime, manager, and worker
pub struct ClareonService {
    runtime: Runtime,
    handle: ServiceHandle,
    worker_handle: JoinHandle<()>,
}

impl ClareonService {
    /// Create a new Clareon service
    ///
    /// This initializes the async runtime, storage, backend, and conversation manager.
    /// It also starts the background worker task.
    ///
    /// Configuration is accessed via the global ConfigManager singleton.
    pub fn new() -> Result<Self> {
        // Create tokio runtime
        let runtime = Runtime::new()?;

        // Initialize core components on the runtime
        let manager = runtime.block_on(async {
            // Get config from singleton
            let config = ConfigManager::get().config();

            // Initialize storage
            let storage = Arc::new(Storage::new(&Config::database_url()?).await?);

            // Create backends
            let backend = clareon_core::backend::create_backend_from_config(&config)
                .await
                .map_err(std::io::Error::other)?;
            let title_backend = clareon_core::backend::create_backend_from_config(&config)
                .await
                .map_err(std::io::Error::other)?;

            let (tool_executor, mcp_manager) = build_tools(&config, Arc::clone(&storage)).await?;

            // Create conversation manager
            let mut manager = ConversationManager::new(
                Arc::clone(&storage),
                Arc::clone(&backend),
                Arc::clone(&title_backend),
                config,
            );

            // Add tool executor if available
            if let Some(executor) = tool_executor {
                manager = manager.with_tools(executor);
            }

            Ok::<_, Error>((manager, mcp_manager))
        })?;

        let (manager, mcp_manager) = manager;

        // Create channels for command/response
        let (command_tx, command_rx) = broadcast::channel(100);
        let (response_tx, _) = broadcast::channel(100);

        // Create and spawn worker
        let worker = ServiceWorker::new(manager, mcp_manager, command_rx, response_tx.clone());
        let worker_handle = runtime.spawn(async move {
            worker.run().await;
        });

        Ok(Self {
            runtime,
            handle: ServiceHandle {
                command_tx,
                response_tx,
            },
            worker_handle,
        })
    }

    /// Get a handle for sending commands
    pub fn handle(&self) -> ServiceHandle {
        self.handle.clone()
    }

    /// Get a reference to the runtime
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Shutdown the service gracefully
    pub fn shutdown(mut self) {
        // Send shutdown command
        let _ = self.handle.send(Command::Shutdown);

        // Take ownership of worker_handle to avoid Drop issues
        let worker_handle =
            std::mem::replace(&mut self.worker_handle, self.runtime.spawn(async {}));

        // Wait for worker to finish (with timeout)
        let _ = self.runtime.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(5), worker_handle).await
        });
    }
}

impl Drop for ClareonService {
    fn drop(&mut self) {
        // Try to send shutdown command
        let _ = self.handle.send(Command::Shutdown);
    }
}
