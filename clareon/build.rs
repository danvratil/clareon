// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn find_qml_files() -> Vec<String> {
    let root_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap().as_str());
    let qml_dir = root_dir.join("qml");
    let files = glob::glob(qml_dir.join("**/*.qml").to_str().unwrap()).unwrap();
    files
        .map(|entry| {
            entry
                .unwrap()
                .strip_prefix(&root_dir)
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("cz.dvratil.clareon")
            .qml_files(find_qml_files())
            .depend("QtQuick"),
    )
    .files([
        "src/models.rs",
        "src/app_controller.rs",
        "src/logging.rs"
    ])
    .qt_module("Quick")
    .cpp_file("src/cpp/logging.cpp")
    .build();
}
