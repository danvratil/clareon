// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use cmake_package::find_package;
use std::path::{Path, PathBuf};

pub fn run_moc(header_path: &PathBuf, output_path: &PathBuf) -> Result<(), String> {
    println!("cargo:rerun-if-env-changed=QT_MOC_EXECUTABLE");
    let moc_path = match std::env::var("QT_MOC_EXECUTABLE") {
        Ok(moc_path) => moc_path,
        Err(_) => {
            let qt6_core = find_package("Qt6Core")
                .find()
                .map_err(|e| format!("Failed to find Qt6Core: {}", e))?;
            let qt6_moc = qt6_core
                .target("Qt6::moc")
                .ok_or_else(|| "Failed to find Qt6::moc target".to_string())?;
            qt6_core
                .target_property(&qt6_moc, "LOCATION")
                .ok_or_else(|| "Failed to get LOCATION property of Qt6::moc".to_string())?
        }
    };

    if Path::new(&moc_path).exists() {
        println!("Found MOC at: {}", moc_path);
    } else {
        return Err(format!(
            "MOC executable not found at location {}",
            moc_path
        ));
    }

    // Run MOC
    let status = std::process::Command::new(&moc_path)
        .arg(header_path)
        .arg("-o")
        .arg(output_path)
        .status()
        .map_err(|e| format!("Failed to run MOC: {}", e))?;

    if !status.success() {
        return Err(format!("MOC failed with status: {}", status));
    }

    println!("Generated MOC file: {}", output_path.display());
    Ok(())
}
