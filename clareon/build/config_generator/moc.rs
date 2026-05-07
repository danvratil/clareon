// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use qt_build_utils::{MocArguments, QtInstallationQMake, QtToolMoc};
use std::path::PathBuf;

pub fn run_moc(header_path: &PathBuf) -> Result<PathBuf, String> {
    let qt_installation =
        QtInstallationQMake::new().map_err(|e| format!("Failed to find Qt installation: {e}"))?;
    let moc = QtToolMoc::new(&qt_installation, &["QtCore".to_string()]);

    let output = moc.compile(header_path, MocArguments::default());

    Ok(output.cpp)
}
