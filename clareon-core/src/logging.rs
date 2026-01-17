// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{fs::File, path::PathBuf};

use crate::Config;
use anyhow::Result;
use tracing_appender::{self, non_blocking::WorkerGuard};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Get the log file path
fn log_file_path() -> Result<PathBuf> {
    let data_dir = directories::ProjectDirs::from("org", "clareon", "clareon")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let log_dir = data_dir.data_dir();
    std::fs::create_dir_all(log_dir)?;
    Ok(log_dir.join("debug.log"))
}

/// A guard for logging resources
#[must_use]
pub struct LoggingGuard {
    _worker_guard: Option<WorkerGuard>,
    path: Option<PathBuf>,
}

impl LoggingGuard {
    fn new_with_guard(worker_guard: WorkerGuard, path: PathBuf) -> Self {
        Self {
            _worker_guard: Some(worker_guard),
            path: Some(path),
        }
    }

    fn empty() -> Self {
        Self {
            _worker_guard: None,
            path: None,
        }
    }
}

/// Initialize file-based logging for TUI mode
pub fn init_logging(config: &Config) -> Result<LoggingGuard> {
    let (non_blocking, guard) = if config.logging.log_to_file {
        let log_path = log_file_path()?;
        let log_file = File::create(&log_path)?;
        let (non_blocking, guard) = tracing_appender::non_blocking(log_file);
        (
            Some(non_blocking),
            LoggingGuard::new_with_guard(guard, log_path),
        )
    } else {
        (None, LoggingGuard::empty())
    };

    // Build filter directive from config
    let default_filter = config.logging.build_filter_directive();

    let builder = tracing_subscriber::registry().with(
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&default_filter)),
    );
    if let Some(non_blocking) = non_blocking {
        builder
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false),
            )
            .init()
    } else {
        builder
            .with(tracing_subscriber::fmt::layer().with_ansi(true))
            .init();
    };

    if let Some(log_path) = &guard.path {
        tracing::info!("Logging initialized to {:?}", log_path);
    } else {
        tracing::info!("Logging initialized");
    }
    tracing::debug!("Log filter: {}", default_filter);

    Ok(guard)
}
