// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("src/cpp/qml.hpp");

        #[cxx_name = "registerClareonQmlTypes"]
        fn register_clareon_qml_types();

        /// Enable QML/JS debug server (QMLMCP / Qt Creator). Call before any QML engine.
        #[cxx_name = "enableQmlDebugger"]
        fn enable_qml_debugger(port: i32) -> bool;
    }
}

pub use ffi::{enable_qml_debugger, register_clareon_qml_types};
