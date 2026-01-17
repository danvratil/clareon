// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use cxx_qt_build::CxxQtBuilder;

fn main() {
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
