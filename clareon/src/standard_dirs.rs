// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-platform standard directory lookups.
//!
//! Provides an `xdg::BaseDirectories`-style API that works on both Unix
//! (using the XDG Base Directory specification via the `xdg` crate) and
//! Windows (using `directories::BaseDirs`, which resolves to the standard
//! `%APPDATA%`/`%LOCALAPPDATA%` Known Folders).

use std::io;
use std::path::{Path, PathBuf};

/// Return the path that a config file should be placed at, creating any
/// missing parent directories.
///
/// On Unix this resolves under `$XDG_CONFIG_HOME` (or `~/.config`).
/// On Windows it resolves under the Roaming `%APPDATA%` directory.
pub fn place_config_file<P: AsRef<Path>>(suffix: P) -> io::Result<PathBuf> {
    let suffix = suffix.as_ref();
    #[cfg(unix)]
    {
        xdg::BaseDirectories::new().place_config_file(suffix)
    }
    #[cfg(windows)]
    {
        place_under(base_config_dir()?, suffix)
    }
}

/// Return the path to an existing config file, or `None` if it doesn't exist.
pub fn get_config_file<P: AsRef<Path>>(suffix: P) -> Option<PathBuf> {
    let suffix = suffix.as_ref();
    #[cfg(unix)]
    {
        xdg::BaseDirectories::new().get_config_file(suffix)
    }
    #[cfg(windows)]
    {
        let path = base_config_dir().ok()?.join(suffix);
        path.exists().then_some(path)
    }
}

#[cfg(windows)]
fn base_config_dir() -> io::Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|b| b.config_dir().to_path_buf())
        .ok_or_else(|| io::Error::other("could not determine user config directory"))
}

#[cfg(windows)]
fn place_under(base: PathBuf, suffix: &Path) -> io::Result<PathBuf> {
    let path = base.join(suffix);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}
