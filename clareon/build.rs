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
        "src/qml.rs",
        "src/logging.rs",
        "src/service_controller.rs",
        "src/conversation_list_model.rs",
        "src/message_list_model.rs",
        "src/search_result_model.rs",
    ])
    .cpp_file("src/cpp/logging.cpp")
    .cpp_file("src/cpp/qml.cpp")
    .qt_module("Quick")
    .qt_module("Qml")
    .build();
}
