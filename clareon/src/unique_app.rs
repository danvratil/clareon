// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unique application instance management.
//!
//! This module ensures only a single instance of Clareon runs at a time.
//! When a second instance is launched, it sends its command-line arguments
//! to the existing instance and exits.
//!
//! # Platform Support
//!
//! - **Unix**: Uses Unix domain sockets at `$XDG_RUNTIME_DIR/clareon.sock`
//!   (falls back to `$TMPDIR` or `/tmp`).
//! - **Windows**: Uses a Named Mutex for atomic primary detection paired with
//!   a Named Pipe at `\\.\pipe\clareon-<USERNAME>` for activation messaging.
//!
//! # Example Usage
//!
//! ```no_run
//! use clareon::unique_app::{try_become_unique, UniqueResult};
//!
//! match try_become_unique() {
//!     Ok(UniqueResult::Primary(server)) => {
//!         // First instance — spawn listener on a Tokio runtime
//!         tokio::spawn(server.listen(|activation| {
//!             println!("Received activation: {:?}", activation.args);
//!         }));
//!     }
//!     Ok(UniqueResult::Secondary) => {
//!         std::process::exit(0);
//!     }
//!     Err(e) => {
//!         eprintln!("Error establishing unique instance: {}", e);
//!     }
//! }
//! ```

use std::env;
use std::io;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use self::unix as imp;
#[cfg(windows)]
use self::windows as imp;

/// Activation message sent from a secondary instance to the primary instance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Activation {
    /// Command-line arguments passed to the secondary instance.
    pub args: Vec<String>,
}

impl Activation {
    /// Create a new activation from this process's command-line arguments.
    pub fn from_args() -> Self {
        Self {
            args: env::args().collect(),
        }
    }

    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Result of attempting to become the unique instance.
pub enum UniqueResult {
    /// This is the primary instance and owns the activation listener.
    Primary(UniqueServer),
    /// Another instance is already running and we sent it our activation.
    Secondary,
}

/// Server that listens for activations from other instances.
pub struct UniqueServer(imp::Server);

impl UniqueServer {
    /// Listen for incoming activations.
    ///
    /// Must be called from within a Tokio runtime. Each activation received
    /// from a secondary instance triggers an invocation of `on_activation`.
    pub async fn listen<F>(self, on_activation: F)
    where
        F: Fn(Activation) + Send + Sync + 'static,
    {
        self.0.listen(on_activation).await
    }
}

/// Try to become the unique application instance.
///
/// Returns:
/// - `Ok(UniqueResult::Primary(server))` if this is the first instance.
/// - `Ok(UniqueResult::Secondary)` if another instance is running and we
///   sent it our activation.
/// - `Err(e)` on failure.
pub fn try_become_unique() -> io::Result<UniqueResult> {
    imp::try_become_unique()
}
