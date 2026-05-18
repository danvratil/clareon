// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use cmake_package::find_package;
use cxx_qt_build::CxxQtBuilder;

/// Find the Qt MOC executable.
///
/// Checks the `QT_MOC_EXECUTABLE` environment variable first, then falls back
/// to using cmake-package to find it via CMake.
fn find_qt_moc() -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=QT_MOC_EXECUTABLE");
    let moc_path = match std::env::var("QT_MOC_EXECUTABLE") {
        Ok(moc_path) => moc_path,
        Err(_) => {
            let qt6_core = find_package("Qt6Core").find().ok()?;
            let qt6_moc = qt6_core.target("Qt6::moc")?;
            qt6_core.target_property(&qt6_moc, "LOCATION")?
        }
    };

    let path = PathBuf::from(&moc_path);
    if path.is_file() {
        return Some(path);
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

    let kiconthemes_pkg = find_package("KF6IconThemes")
        .find()
        .expect("Could not find KF6IconThemes package via CMake");
    let kicontheme_tgt = kiconthemes_pkg
        .target("KF6::IconThemes")
        .expect("Could not find KF6::IconThemes target in CMake package");
    kicontheme_tgt.link();

    let mut builder = CxxQtBuilder::new()
        .file("src/qicon.rs")
        .file("src/qapplication.rs")
        .file("src/kiconthemes.rs")
        .cpp_file("src/cpp/qicon.cpp")
        .cpp_file("src/cpp/qapplication.cpp")
        .cpp_file("src/cpp/kiconthemes.cpp")
        .include_dir("include")
        .qt_module("Gui")
        .qt_module("Widgets");

    unsafe {
        builder = builder.cc_builder(|cc| {
            cc.includes(kicontheme_tgt.include_directories.clone());
        });
    }

    builder.build();
}
