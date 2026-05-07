// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Windows-specific implementation.
//!
//! Two OS primitives cooperate here:
//!
//! - A **named mutex** is used for atomic primary-instance detection. The
//!   handle is acquired synchronously (no Tokio runtime is available yet) and
//!   held for the lifetime of the primary process.
//! - A **named pipe** carries the activation payload. It is created later,
//!   inside the Tokio runtime, because `tokio::net::windows::named_pipe` only
//!   functions within a runtime context.
//!
//! Secondary instances detect the mutex via `ERROR_ALREADY_EXISTS`, then open
//! the named pipe as a regular file to deliver the activation. A short retry
//! loop covers the brief window between the primary acquiring the mutex and
//! the runtime spinning up the pipe.

use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tracing::{debug, error, info, warn};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;

use super::{Activation, UniqueResult, UniqueServer};

/// RAII wrapper that closes a Windows handle on drop.
struct OwnedHandle(HANDLE);

// SAFETY: Windows HANDLEs are kernel objects that may be used from any thread.
unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 is a handle returned by a successful Win32 call
            // and has not been closed elsewhere.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

pub(super) struct Server {
    /// Mutex handle that proves we are the primary instance. Released on drop.
    _mutex: OwnedHandle,
}

impl Server {
    pub(super) async fn listen<F>(self, on_activation: F)
    where
        F: Fn(Activation) + Send + Sync + 'static,
    {
        let on_activation = Arc::new(on_activation);
        let pipe_name = pipe_name();
        info!("Unique server listening on named pipe {}", pipe_name);

        let mut server = match ServerOptions::new().create(&pipe_name) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create initial named pipe instance: {}", e);
                return;
            }
        };

        loop {
            if let Err(e) = server.connect().await {
                error!("Error accepting connection: {}", e);
                break;
            }
            debug!("Received connection from client");

            // Hand off the connected pipe and spin up a fresh instance so the
            // next client can connect immediately.
            let connected = server;
            server = match ServerOptions::new().create(&pipe_name) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create successor pipe instance: {}", e);
                    let on_activation = Arc::clone(&on_activation);
                    if let Err(e) = handle_client(connected, on_activation).await {
                        error!("Error handling client: {}", e);
                    }
                    break;
                }
            };

            let on_activation = Arc::clone(&on_activation);
            tokio::spawn(async move {
                if let Err(e) = handle_client(connected, on_activation).await {
                    error!("Error handling client: {}", e);
                }
            });
        }
    }
}

async fn handle_client<F>(stream: NamedPipeServer, on_activation: Arc<F>) -> io::Result<()>
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

fn user_suffix() -> String {
    env::var("USERNAME").unwrap_or_else(|_| "default".to_string())
}

fn pipe_name() -> String {
    format!(r"\\.\pipe\clareon-{}", user_suffix())
}

fn mutex_name_wide() -> Vec<u16> {
    // The `Local\` namespace scopes the mutex to the current login session,
    // so multiple users on the same machine each get their own primary.
    let name = format!(r"Local\clareon-mutex-{}", user_suffix());
    OsStr::new(&name).encode_wide().chain(Some(0)).collect()
}

pub(super) fn try_become_unique() -> io::Result<UniqueResult> {
    let name_w = mutex_name_wide();

    // SAFETY: name_w is a NUL-terminated UTF-16 string owned by this scope.
    let handle = unsafe { CreateMutexW(ptr::null(), 0, name_w.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mutex = OwnedHandle(handle);

    // SAFETY: GetLastError is always safe to call.
    let last_error = unsafe { GetLastError() };
    if last_error == ERROR_ALREADY_EXISTS {
        debug!("Found existing instance via named mutex, sending activation");
        match try_activate_existing() {
            Ok(()) => {
                info!("Activated existing instance, exiting");
                Ok(UniqueResult::Secondary)
            }
            Err(e) => {
                warn!("Error trying to activate existing instance: {}", e);
                Err(e)
            }
        }
    } else {
        debug!("Acquired primary instance mutex");
        Ok(UniqueResult::Primary(UniqueServer(Server {
            _mutex: mutex,
        })))
    }
}

fn try_activate_existing() -> io::Result<()> {
    let pipe_name = pipe_name();

    // The primary instance creates the named pipe lazily once its Tokio
    // runtime is up, so there is a brief window where the mutex exists but
    // the pipe does not. Retry on `NotFound` to absorb that window.
    const MAX_ATTEMPTS: u32 = 30;
    const RETRY_INTERVAL: Duration = Duration::from_millis(200);

    let mut stream = {
        let mut attempt = 0;
        loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&pipe_name)
            {
                Ok(f) => break f,
                Err(e) if e.kind() == io::ErrorKind::NotFound && attempt < MAX_ATTEMPTS => {
                    attempt += 1;
                    std::thread::sleep(RETRY_INTERVAL);
                }
                Err(e) => return Err(e),
            }
        }
    };

    let activation = Activation::from_args();
    let json = activation
        .to_json()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("JSON error: {}", e)))?;

    stream.write_all(json.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    debug!("Sent activation to existing instance");
    Ok(())
}
