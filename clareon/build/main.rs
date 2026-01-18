// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

mod config_generator;

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
    // Generate C++ config code from Rust settings
    let config = config_generator::generate_config_cpp();

    // Get OUT_DIR for include path
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    CxxQtBuilder::new_qml_module(
        QmlModule::new("cz.dvratil.clareon")
            .qml_files(find_qml_files())
            .depend("QtQuick"),
    )
    .qrc("icons/icons.qrc")
    .files([
        "src/qml.rs",
        "src/logging.rs",
        "src/service_controller.rs",
        "src/conversation_list_model.rs",
        "src/message_list_model.rs",
        "src/search_result_model.rs",
        "src/config_manager.rs",
    ])
    .cpp_file("src/cpp/logging.cpp")
    .cpp_file("src/cpp/qml.cpp")
    .cpp_file(config.impl_path) // Add generated config implementation
    .cpp_file(config.moc_path) // Add MOC-generated file
    .include_dir(&out_dir) // Add OUT_DIR to include path for config_generated.h
    .include_dir("src/cpp") // Add src/cpp to include path for config_bridge.hpp
    .include_dir(
        config
            .header_path
            .parent()
            .expect("Missing include header path parent"),
    )
    .qt_module("Quick")
    .qt_module("Qml")
    .build();
}
