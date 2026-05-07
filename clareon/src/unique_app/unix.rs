// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unix-specific implementation backed by a filesystem Unix domain socket.

use std::env;
use std::io::{self, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener as TokioUnixListener, UnixStream as TokioUnixStream};
use tracing::{debug, error, info, warn};

use super::{Activation, UniqueResult, UniqueServer};

pub(super) struct Server {
    listener: UnixListener,
}

impl Server {
    pub(super) async fn listen<F>(self, on_activation: F)
    where
        F: Fn(Activation) + Send + Sync + 'static,
    {
        let on_activation = Arc::new(on_activation);
        info!("Unique server started, listening for activations");
        let listener = TokioUnixListener::from_std(self.listener)
            .expect("Failed to convert std listener to Tokio");
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    debug!("Received connection from client");
                    let on_activation = Arc::clone(&on_activation);
                    if let Err(e) = handle_client(stream, on_activation).await {
                        error!("Error handling client: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                    break;
                }
            }
        }

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

fn socket_path() -> PathBuf {
    let runtime_dir = env::var("XDG_RUNTIME_DIR")
        .or_else(|_| env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let path = PathBuf::from(runtime_dir).join("clareon.sock");
    debug!("Using filesystem socket: {:?}", path);
    path
}

pub(super) fn try_become_unique() -> io::Result<UniqueResult> {
    let path = socket_path();

    match try_activate_existing(&path) {
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

    create_server(&path)
}

fn try_activate_existing(path: &Path) -> io::Result<bool> {
    match UnixStream::connect(path) {
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

fn create_server(path: &Path) -> io::Result<UniqueResult> {
    if path.exists() {
        debug!("Removing stale socket file: {:?}", path);
        std::fs::remove_file(path)?;
    }

    let listener = UnixListener::bind(path)?;
    listener.set_nonblocking(true)?;

    Ok(UniqueResult::Primary(UniqueServer(Server { listener })))
}
