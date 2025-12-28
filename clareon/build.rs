// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("cz.dvratil.clareon")
            .qml_files([
                "qml/main.qml",
                "qml/ChatView.qml",
                "qml/ConversationDrawer.qml",
                "qml/MessageDelegate.qml",
                "qml/MessageComposer.qml",
            ])
            .depend("QtQuick"),
    )
    .files(["src/models.rs", "src/app_controller.rs"])
    .qt_module("Quick")
    .build();
}
