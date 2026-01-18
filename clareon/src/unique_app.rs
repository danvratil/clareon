// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unique application instance management using named pipes.
//!
//! This module ensures only a single instance of Clareon runs at a time.
//! When a second instance is launched, it sends its command-line arguments
//! to the existing instance and exits.
//!
//! # Platform Support
//!
//! - **Linux**: Uses abstract namespace Unix sockets (no filesystem entry, automatic cleanup)
//! - **Other Unix**: Uses Unix domain sockets in `$XDG_RUNTIME_DIR/clareon.sock`
//! - **Windows**: Not yet supported (future integration point)
//!
//! Abstract namespace sockets (Linux-only) are preferred because they:
//! - Don't create files in the filesystem
//! - Are automatically cleaned up when the process exits or crashes
//! - Avoid issues with stale socket files
//!
//! # Example Usage
//!
//! ```no_run
//! use clareon::unique_app::{try_become_unique, UniqueResult};
//!
//! #[tokio::main]
//! async fn main() {
//!     match try_become_unique().await {
//!         Ok(UniqueResult::Primary(server)) => {
//!             // This is the first instance, spawn listener task
//!             let _handle = server.listen(|activation| {
//!                 println!("Received activation: {:?}", activation.args);
//!                 // Handle activation (e.g., show window, process args)
//!             });
//!             // Continue with application initialization
//!         }
//!         Ok(UniqueResult::Secondary) => {
//!             // Another instance is running, we sent our args and can exit
//!             std::process::exit(0);
//!         }
//!         Err(e) => {
//!             eprintln!("Error establishing unique instance: {}", e);
//!             // Optionally continue without unique instance management
//!         }
//!     }
//! }
//! ```

use std::env;
use std::io;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener as TokioUnixListener, UnixStream as TokioUnixStream};
use tracing::{debug, error, info, warn};

/// Activation message sent from client to server
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Activation {
    /// Command-line arguments passed to the application
    pub args: Vec<String>,
}

impl Activation {
    /// Create a new activation from command-line arguments
    pub fn from_args() -> Self {
        Self {
            args: env::args().collect(),
        }
    }

    /// Serialize to JSON string
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON string
    fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Result of attempting to become the unique instance
pub enum UniqueResult {
    /// This is the primary instance (server started successfully)
    Primary(UniqueServer),
    /// Another instance is already running (activation sent successfully)
    Secondary,
}

/// Server that listens for activations from other instances
pub struct UniqueServer {
    listener: UnixListener,
}

impl UniqueServer {
    /// Listen for incoming activations in a background task
    ///
    /// Spawns an async task that listens for connections and invokes the callback
    /// for each activation received. Returns a handle that can be used to await
    /// the task completion if needed.
    ///
    /// The callback is invoked for each activation received.
    pub async fn listen<F>(self, on_activation: F)
    where
        F: Fn(Activation) + Send + Sync + 'static,
    {
        let on_activation = Arc::new(on_activation);
        info!("Unique server started, listening for activations");
        let listener = TokioUnixListener::from_std(self.listener)
            .expect("Failed to conveert std listener to Tokio");
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    debug!("Received connection from client");
                    let on_activation = Arc::clone(&on_activation);
                    if let Err(e) = Self::handle_client(stream, on_activation).await {
                        error!("Error handling client: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                    break;
                }
            }
        }

        {
            if let Some(path) = listener
                .local_addr()
                .ok()
                .and_then(|addr| addr.as_pathname().map(|path| path.to_owned()))
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    async fn handle_client<F>(stream: TokioUnixStream, on_activation: Arc<F>) -> io::Result<()>
    where
        F: Fn(Activation) + Send + Sync,
    {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        match Activation::from_json(&line) {
            Ok(activation) => {
                debug!("Received activation: {:?}", activation);
                on_activation(activation);
                Ok(())
            }
            Err(e) => {
                error!("Failed to parse activation: {}", e);
                Err(io::Error::new(io::ErrorKind::InvalidData, e))
            }
        }
    }
}

/// Get the socket path/address
fn get_socket_path() -> std::path::PathBuf {
    let runtime_dir = env::var("XDG_RUNTIME_DIR")
        .or_else(|_| env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let path = std::path::PathBuf::from(runtime_dir).join("clareon.sock");
    debug!("Using filesystem socket: {:?}", path);
    path
}

/// Try to become the unique application instance
///
/// Returns:
/// - `Ok(UniqueResult::Primary(server))` if this is the first instance
/// - `Ok(UniqueResult::Secondary)` if another instance is running (activation sent)
/// - `Err(e)` if there was an error
pub fn try_become_unique() -> io::Result<UniqueResult> {
    let socket_path = get_socket_path();

    // First, try to connect as a client
    match try_activate_existing(&socket_path) {
        Ok(true) => {
            info!("Activated existing instance, exiting");
            return Ok(UniqueResult::Secondary);
        }
        Ok(false) => {
            debug!("No existing instance found, becoming primary");
        }
        Err(e) => {
            warn!("Error trying to activate existing instance: {}", e);
        }
    }

    // If we couldn't connect, become the server
    create_server(&socket_path)
}

/// Try to activate an existing instance
///
/// Returns:
/// - `Ok(true)` if activation was sent successfully
/// - `Ok(false)` if no instance is running
/// - `Err(e)` on error
fn try_activate_existing(socket_path: &PathBuf) -> io::Result<bool> {
    match UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            let activation = Activation::from_args();
            let json = activation.to_json().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("JSON error: {}", e))
            })?;

            stream.write_all(json.as_bytes())?;
            stream.write_all(b"\n")?;
            stream.flush()?;

            debug!("Sent activation to existing instance");
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Create a new server instance
fn create_server(socket_path: &PathBuf) -> io::Result<UniqueResult> {
    // For filesystem-based sockets (non-Linux), remove stale files
    // Abstract namespace sockets (Linux) don't create files
    if socket_path.exists() {
        debug!("Removing stale socket file: {:?}", socket_path);
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    listener.set_nonblocking(true)?;

    Ok(UniqueResult::Primary(UniqueServer { listener }))
}
