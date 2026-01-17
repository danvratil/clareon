// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

pub fn run_moc(header_path: &PathBuf, output_path: &PathBuf) -> Result<(), String> {
    // Find Qt installation
    let qt_path = std::env::var("QT_PATH")
        .or_else(|_| std::env::var("QTDIR"))
        .or_else(|_| std::env::var("Qt6_DIR"))
        .unwrap_or_else(|_| "/usr".to_string());

    // Try common MOC locations (prioritize Qt6 versions)
    let moc_candidates = vec![
        PathBuf::from("/usr/lib/qt6/libexec/moc"), // Most common Qt6 location on Linux
        PathBuf::from("/usr/lib/qt6/bin/moc"),
        PathBuf::from("/usr/bin/moc6"), // Some distros use moc6
        PathBuf::from("/usr/bin/moc-qt6"),
        PathBuf::from(&qt_path).join("libexec/moc"),
        PathBuf::from(&qt_path).join("bin/moc"),
        PathBuf::from("/usr/bin/moc"), // Fallback to generic moc (might be Qt5)
    ];

    let moc_path = moc_candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "Could not find MOC executable".to_string())?;

    println!("Found MOC at: {}", moc_path.display());

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

