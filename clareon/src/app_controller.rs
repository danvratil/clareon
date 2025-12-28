// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#[cxx_qt::bridge]
mod app_controller {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i64, current_conversation_id, cxx_name = "currentConversationId")]
        #[qproperty(QString, conversation_title, cxx_name = "conversationTitle")]
        #[qproperty(bool, is_waiting, cxx_name = "isWaiting")]
        #[qproperty(QString, status_message, cxx_name = "statusMessage")]
        #[qproperty(QString, view_mode, cxx_name = "viewMode")]
        type AppController = super::AppControllerRust;

        #[qsignal]
        #[cxx_name = "conversationChanged"]
        fn conversation_changed(self: Pin<&mut AppController>);

        #[qsignal]
        fn error(self: Pin<&mut AppController>, message: QString);
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        #[cxx_name = "selectConversation"]
        fn select_conversation(self: Pin<&mut AppController>, conversation_id: i64);

        #[qinvokable]
        #[cxx_name = "sendMessage"]
        fn send_message(self: Pin<&mut AppController>, text: &QString);

        #[qinvokable]
        #[cxx_name = "newConversation"]
        fn new_conversation(self: Pin<&mut AppController>);

        #[qinvokable]
        fn search(self: Pin<&mut AppController>, query: &QString);
    }
}

use std::pin::Pin;

pub struct AppControllerRust {
    current_conversation_id: i64,
    conversation_title: cxx_qt_lib::QString,
    is_waiting: bool,
    status_message: cxx_qt_lib::QString,
    view_mode: cxx_qt_lib::QString,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            current_conversation_id: 1,
            conversation_title: cxx_qt_lib::QString::from("Rust async patterns"),
            is_waiting: false,
            status_message: cxx_qt_lib::QString::from("Ready"),
            view_mode: cxx_qt_lib::QString::from("chat"),
        }
    }
}

impl app_controller::AppController {
    pub fn select_conversation(mut self: Pin<&mut Self>, conversation_id: i64) {
        let title = match conversation_id {
            1 => "Rust async patterns",
            2 => "QML layout design",
            3 => "Debugging SQLite queries",
            4 => "Kirigami components overview",
            5 => "Git workflow best practices",
            6 => "Linux desktop integration",
            7 => "Error handling in Rust",
            8 => "CSS grid vs flexbox",
            _ => "Conversation",
        };

        self.as_mut().set_current_conversation_id(conversation_id);
        self.as_mut()
            .set_conversation_title(cxx_qt_lib::QString::from(title));
        self.as_mut().conversation_changed();
    }

    pub fn send_message(mut self: Pin<&mut Self>, text: &cxx_qt_lib::QString) {
        let message_text = text.to_string();

        if message_text.trim().is_empty() {
            return;
        }

        self.as_mut().set_is_waiting(true);
        self.as_mut()
            .set_status_message(cxx_qt_lib::QString::from("Sending message..."));

        // In real app, this would send to backend

        self.as_mut().set_is_waiting(false);
        self.as_mut()
            .set_status_message(cxx_qt_lib::QString::from("Ready"));
    }

    pub fn new_conversation(mut self: Pin<&mut Self>) {
        let new_id = 100;
        self.as_mut().set_current_conversation_id(new_id);
        self.as_mut()
            .set_conversation_title(cxx_qt_lib::QString::from("New Conversation"));
        self.as_mut()
            .set_status_message(cxx_qt_lib::QString::from("Started new conversation"));
        self.as_mut().conversation_changed();
    }

    pub fn search(mut self: Pin<&mut Self>, query: &cxx_qt_lib::QString) {
        let query_text = query.to_string();

        if query_text.trim().is_empty() {
            self.as_mut().set_view_mode(cxx_qt_lib::QString::from("chat"));
            return;
        }

        self.as_mut().set_view_mode(cxx_qt_lib::QString::from("search"));
        self.as_mut().set_status_message(cxx_qt_lib::QString::from(
            format!("Searching for '{}'...", query_text).as_str(),
        ));
    }
}
