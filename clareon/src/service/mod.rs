// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service layer providing a bridge between clareon-core and the Qt UI

mod command;
mod response;
mod worker;

pub use command::Command;
pub use response::{ArtifactData, ErrorCategory, ErrorInfo, MessageData, Response};
pub use worker::ServiceWorker;

use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use clareon_core::{
    ConfigManager, ConversationManager, Error, Result, Storage,
    tools::{
        ArtifactManager, BubblewrapSandbox, NoneSandbox, SandboxMode, ToolExecutor, ToolRegistry,
        WorkspaceManager, register_builtin_tools,
    },
};

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
    /// Create a new Clareon service for a specific profile
    ///
    /// This initializes the async runtime, storage, backend, and conversation manager.
    /// It also starts the background worker task.
    pub fn new(config_manager: Arc<ConfigManager>) -> Result<Self> {
        // Create tokio runtime
        let runtime = Runtime::new()?;

        // Initialize core components on the runtime
        let manager = runtime.block_on(async {
            let config = config_manager.config();
            let profile = config_manager.profile();

            // Initialize storage using profile's database URL
            let storage = Arc::new(Storage::new(&profile.database_url).await?);

            // Create backends, passing profile ID for secret retrieval
            let backend = clareon_core::backend::create_backend_from_config(&config, &profile.id)
                .await
                .map_err(std::io::Error::other)?;
            let title_backend =
                clareon_core::backend::create_backend_from_config(&config, &profile.id)
                    .await
                    .map_err(std::io::Error::other)?;

            // Create tool executor if tools are enabled
            let tool_executor = if config.tools.enabled {
                // Create tool registry with built-in tools
                let mut registry = ToolRegistry::default();
                register_builtin_tools(&mut registry);
                let registry = Arc::new(registry);

                // Create sandbox based on config
                use clareon_core::config::SandboxModeConfig;
                let sandbox: Arc<dyn clareon_core::tools::Sandbox> = match config.tools.sandbox_mode
                {
                    SandboxModeConfig::Strict => {
                        Arc::new(BubblewrapSandbox::new(SandboxMode::Strict))
                    }
                    SandboxModeConfig::Basic => {
                        Arc::new(BubblewrapSandbox::new(SandboxMode::Basic))
                    }
                    SandboxModeConfig::None => Arc::new(NoneSandbox),
                };

                // Use profile's workspace cache directory
                let workspace_dir = profile.workspace_cache_dir.clone();

                // Create workspace manager
                let workspace_manager =
                    Arc::new(WorkspaceManager::new(workspace_dir, Arc::clone(&storage)));

                // Create artifact manager (shares storage with workspace manager)
                let artifact_manager = Arc::new(ArtifactManager::new(Arc::clone(&storage)));

                // Create tool executor
                let executor =
                    ToolExecutor::new(registry, sandbox, workspace_manager, artifact_manager);

                Some(Arc::new(executor))
            } else {
                None
            };

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

            Ok::<_, Error>(manager)
        })?;

        // Create channels for command/response
        let (command_tx, command_rx) = broadcast::channel(100);
        let (response_tx, _) = broadcast::channel(100);

        // Create and spawn worker
        let worker = ServiceWorker::new(manager, command_rx, response_tx.clone());
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
