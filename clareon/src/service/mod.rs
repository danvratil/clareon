// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service layer providing a bridge between clareon-core and the Qt UI

mod command;
mod response;
mod worker;

pub use command::Command;
pub use response::{MessageData, Response};
pub use worker::ServiceWorker;

use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use clareon_core::{Config, ConversationManager, Error, Result, Storage};

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
    pub fn new(config: Config) -> Result<Self> {
        // Create tokio runtime
        let runtime = Runtime::new()?;

        // Initialize core components on the runtime
        let manager = runtime.block_on(async {
            // Initialize storage
            let storage = Storage::new(&Config::database_url()?).await?;

            // Create backends
            let backend = clareon_core::backend::create_backend_from_config(&config)
                .await
                .map_err(std::io::Error::other)?;
            let title_backend = clareon_core::backend::create_backend_from_config(&config)
                .await
                .map_err(std::io::Error::other)?;

            // Create conversation manager
            let manager = ConversationManager::new(
                storage,
                Arc::clone(&backend),
                Arc::clone(&title_backend),
                config,
            );

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
