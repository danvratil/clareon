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
        #[qproperty(QString, current_conversation_id, cxx_name = "currentConversationId")]
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
        fn select_conversation(self: Pin<&mut AppController>, conversation_id: &QString);

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
    current_conversation_id: cxx_qt_lib::QString,
    conversation_title: cxx_qt_lib::QString,
    is_waiting: bool,
    status_message: cxx_qt_lib::QString,
    view_mode: cxx_qt_lib::QString,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            current_conversation_id: cxx_qt_lib::QString::from(
                "550e8400-e29b-41d4-a716-446655440001",
            ),
            conversation_title: cxx_qt_lib::QString::from("Rust async patterns"),
            is_waiting: false,
            status_message: cxx_qt_lib::QString::from("Ready"),
            view_mode: cxx_qt_lib::QString::from("chat"),
        }
    }
}

impl app_controller::AppController {
    pub fn select_conversation(mut self: Pin<&mut Self>, conversation_id: &cxx_qt_lib::QString) {
        let id_str = conversation_id.to_string();
        let title = match id_str.as_str() {
            "550e8400-e29b-41d4-a716-446655440001" => "Rust async patterns",
            "550e8400-e29b-41d4-a716-446655440002" => "QML layout design",
            "550e8400-e29b-41d4-a716-446655440003" => "Debugging SQLite queries",
            "550e8400-e29b-41d4-a716-446655440004" => "Kirigami components overview",
            "550e8400-e29b-41d4-a716-446655440005" => "Git workflow best practices",
            "550e8400-e29b-41d4-a716-446655440006" => "Linux desktop integration",
            "550e8400-e29b-41d4-a716-446655440007" => "Error handling in Rust",
            "550e8400-e29b-41d4-a716-446655440008" => "CSS grid vs flexbox",
            _ => "Conversation",
        };

        self.as_mut()
            .set_current_conversation_id(conversation_id.clone());
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
        let new_id = cxx_qt_lib::QString::from("0000000-0000-0000-0000-000000000000");
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
            self.as_mut()
                .set_view_mode(cxx_qt_lib::QString::from("chat"));
            return;
        }

        self.as_mut()
            .set_view_mode(cxx_qt_lib::QString::from("search"));
        self.as_mut().set_status_message(cxx_qt_lib::QString::from(
            format!("Searching for '{}'...", query_text).as_str(),
        ));
    }
}
