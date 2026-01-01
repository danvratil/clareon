// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! MessageListModel - Qt wrapper for messages in a conversation

use std::pin::Pin;

use cxx_qt_lib::QString;

use clareon_core::types::ConversationId;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, conversation_id)]
        #[qproperty(i32, count)]
        type MessageListModel = super::MessageListModelRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut MessageListModel>);

        #[qinvokable]
        fn get_id(self: &MessageListModel, index: i32) -> i64;

        #[qinvokable]
        fn get_role(self: &MessageListModel, index: i32) -> QString;

        #[qinvokable]
        fn get_text(self: &MessageListModel, index: i32) -> QString;

        #[qinvokable]
        fn get_created_at(self: &MessageListModel, index: i32) -> i64;

        #[qsignal]
        fn data_changed(self: Pin<&mut MessageListModel>);
    }

    impl cxx_qt::Threading for MessageListModel {}
}

/// Rust implementation of MessageListModel
#[derive(Default)]
pub struct MessageListModelRust {
    conversation_id: QString,
    count: i32,
}

impl ffi::MessageListModel {
    /// Refresh the model (re-reads from cache)
    pub fn refresh(mut self: Pin<&mut Self>) {
        let conv_id_str = self.conversation_id().to_string();
        if !conv_id_str.is_empty() {
            let conv_id = ConversationId::from(conv_id_str);
            let count = crate::qt::messages_cache()
                .lock()
                .unwrap()
                .get(&conv_id)
                .map(|msgs| msgs.len() as i32)
                .unwrap_or(0);
            self.as_mut().set_count(count);
        } else {
            self.as_mut().set_count(0);
        }
        self.as_mut().data_changed();
    }

    pub fn get_id(&self, index: i32) -> i64 {
        if index < 0 {
            return 0;
        }
        let conv_id_str = self.conversation_id().to_string();
        if conv_id_str.is_empty() {
            return 0;
        }

        let conv_id = ConversationId::from(conv_id_str);
        let cache = crate::qt::messages_cache();
        let messages = cache.lock().unwrap();
        messages
            .get(&conv_id)
            .and_then(|msgs| msgs.get(index as usize))
            .map(|m| m.id)
            .unwrap_or(0)
    }

    pub fn get_role(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let conv_id_str = self.conversation_id().to_string();
        if conv_id_str.is_empty() {
            return QString::default();
        }

        let conv_id = ConversationId::from(conv_id_str);
        let cache = crate::qt::messages_cache();
        let messages = cache.lock().unwrap();
        messages
            .get(&conv_id)
            .and_then(|msgs| msgs.get(index as usize))
            .map(|m| QString::from(&m.role))
            .unwrap_or_default()
    }

    pub fn get_text(&self, index: i32) -> QString {
        if index < 0 {
            return QString::default();
        }
        let conv_id_str = self.conversation_id().to_string();
        if conv_id_str.is_empty() {
            return QString::default();
        }

        let conv_id = ConversationId::from(conv_id_str);
        let cache = crate::qt::messages_cache();
        let messages = cache.lock().unwrap();
        messages
            .get(&conv_id)
            .and_then(|msgs| msgs.get(index as usize))
            .map(|m| QString::from(&m.text))
            .unwrap_or_default()
    }

    pub fn get_created_at(&self, index: i32) -> i64 {
        if index < 0 {
            return 0;
        }
        let conv_id_str = self.conversation_id().to_string();
        if conv_id_str.is_empty() {
            return 0;
        }

        let conv_id = ConversationId::from(conv_id_str);
        let cache = crate::qt::messages_cache();
        let messages = cache.lock().unwrap();
        messages
            .get(&conv_id)
            .and_then(|msgs| msgs.get(index as usize))
            .map(|m| m.created_at)
            .unwrap_or(0)
    }
}

impl cxx_qt::Initialize for ffi::MessageListModel {
    fn initialize(self: Pin<&mut Self>) {
        // Will refresh when conversation_id is set
    }
}
