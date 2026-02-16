// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use cxx_qt_build::CxxQtBuilder;

/// Find the Qt MOC executable.
///
/// Checks the `QT_MOC_EXECUTABLE` environment variable first, then falls back
/// to searching `PATH` for `moc`, `moc-qt6`, and `moc6` (in that order).
fn find_qt_moc() -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=QT_MOC_EXECUTABLE");

    if let Ok(moc_path) = std::env::var("QT_MOC_EXECUTABLE") {
        let path = PathBuf::from(&moc_path);
        if path.is_file() {
            println!("cargo:warning=Using MOC from QT_MOC_EXECUTABLE: {moc_path}");
            return Some(path);
        }
        println!(
            "cargo:warning=QT_MOC_EXECUTABLE is set to '{moc_path}' but the file was not found, \
             falling back to PATH search"
        );
    }

    let path_var = std::env::var_os("PATH")?;
    for candidate in ["moc", "moc-qt6", "moc6"] {
        if let Some(path) = std::env::split_paths(&path_var).find_map(|dir| {
            let full_path = dir.join(candidate);
            full_path.is_file().then_some(full_path)
        }) {
            return Some(path);
        }
    }

    None
}

/// Try to find qmake in the same directory as the given MOC executable.
///
/// This allows cxx-qt-build (which discovers Qt tools via qmake) to use the
/// correct Qt installation matching the MOC we found.
fn find_qmake_from_moc(moc_path: &Path) -> Option<PathBuf> {
    let moc_dir = moc_path.parent()?;
    for candidate in ["qmake6", "qmake-qt6", "qmake"] {
        let qmake_path = moc_dir.join(candidate);
        if qmake_path.is_file() {
            return Some(qmake_path);
        }
    }
    None
}

fn main() {
    // Find MOC and, if QMAKE is not already set, derive the qmake path from
    // MOC's location so that cxx-qt-build discovers the correct Qt installation.
    if let Some(moc_path) = find_qt_moc()
        && std::env::var_os("QMAKE").is_none()
        && let Some(qmake_path) = find_qmake_from_moc(&moc_path)
    {
        // SAFETY: build scripts are single-threaded.
        unsafe {
            std::env::set_var("QMAKE", &qmake_path);
        }
    }

    CxxQtBuilder::new()
        .file("src/qicon.rs")
        .file("src/qapplication.rs")
        .cpp_file("src/cpp/qicon.cpp")
        .cpp_file("src/cpp/qapplication.cpp")
        .include_dir("include")
        .qt_module("Gui")
        .qt_module("Widgets")
        .build();
}
