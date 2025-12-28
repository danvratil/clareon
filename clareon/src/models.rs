// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

    }

    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type MessageListModel = super::MessageListModelRust;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &MessageListModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &MessageListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_name = "roleNames"]
        #[cxx_override]
        fn role_names(self: &MessageListModel) -> QHash_i32_QByteArray;

        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut MessageListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut MessageListModel>);

        #[qinvokable]
        #[cxx_name = "appendMessage"]
        fn append_message(self: Pin<&mut MessageListModel>, role: &QString, text: &QString);
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type ConversationListModel = super::ConversationListModelRust;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &ConversationListModel, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_override]
        fn data(self: &ConversationListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &ConversationListModel) -> QHash_i32_QByteArray;
    }
}

use crate::mock::data::{mock_messages, MockMessage, mock_conversations, MockConversation};
use cxx_qt::CxxQtType;
use std::pin::Pin;

pub struct MessageListModelRust {
    messages: Vec<MockMessage>,
}

impl Default for MessageListModelRust {
    fn default() -> Self {
        Self {
            messages: mock_messages(),
        }
    }
}

impl ffi::MessageListModel {
    pub fn row_count(&self, _parent: &cxx_qt_lib::QModelIndex) -> i32 {
        self.messages.len() as i32
    }

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        let row = index.row();

        if row < 0 || row >= self.messages.len() as i32 {
            return cxx_qt_lib::QVariant::default();
        }

        let message = &self.messages[row as usize];

        match role {
            257 => cxx_qt_lib::QVariant::from(&(message.id as i32)),
            258 => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&message.role)),
            259 => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&message.text_content)),
            260 => cxx_qt_lib::QVariant::from(&(message.created_at as i32)),
            _ => cxx_qt_lib::QVariant::default(),
        }
    }

    pub fn role_names(&self) -> cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray> {
        let mut roles = cxx_qt_lib::QHash::<cxx_qt_lib::QHashPair_i32_QByteArray>::default();
        roles.insert(257, cxx_qt_lib::QByteArray::from("messageId"));
        roles.insert(258, cxx_qt_lib::QByteArray::from("role"));
        roles.insert(259, cxx_qt_lib::QByteArray::from("textContent"));
        roles.insert(260, cxx_qt_lib::QByteArray::from("createdAt"));
        roles
    }

    pub fn append_message(
        mut self: Pin<&mut Self>,
        role: &cxx_qt_lib::QString,
        text: &cxx_qt_lib::QString,
    ) {
        let role_str = role.to_string();
        let text_str = text.to_string();

        let new_id = self.messages.last().map(|m| m.id + 1).unwrap_or(1);
        let created_at = chrono::Utc::now().timestamp();

        let new_message = MockMessage {
            id: new_id,
            role: role_str,
            text_content: text_str,
            created_at,
        };

        let row = self.messages.len() as i32;

        unsafe {
            self.as_mut().begin_insert_rows(&cxx_qt_lib::QModelIndex::default(), row, row);
        }
        self.as_mut().rust_mut().messages.push(new_message);
        unsafe {
            self.as_mut().end_insert_rows();
        }
    }
}

pub struct ConversationListModelRust {
    conversations: Vec<MockConversation>,
}

impl Default for ConversationListModelRust {
    fn default() -> Self {
        Self {
            conversations: mock_conversations(),
        }
    }
}

impl ffi::ConversationListModel {
    pub fn row_count(&self, _parent: &cxx_qt_lib::QModelIndex) -> i32 {
        self.conversations.len() as i32
    }

    pub fn data(&self, index: &cxx_qt_lib::QModelIndex, role: i32) -> cxx_qt_lib::QVariant {
        let row = index.row();

        if row < 0 || row >= self.conversations.len() as i32 {
            return cxx_qt_lib::QVariant::default();
        }

        let conversation = &self.conversations[row as usize];

        match role {
            257 => cxx_qt_lib::QVariant::from(&(conversation.id as i32)),
            258 => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&conversation.title)),
            259 => cxx_qt_lib::QVariant::from(&(conversation.updated_at as i32)),
            260 => cxx_qt_lib::QVariant::from(&cxx_qt_lib::QString::from(&conversation.model)),
            261 => cxx_qt_lib::QVariant::from(&(conversation.message_count as i32)),
            _ => cxx_qt_lib::QVariant::default(),
        }
    }

    pub fn role_names(&self) -> cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray> {
        let mut roles = cxx_qt_lib::QHash::<cxx_qt_lib::QHashPair_i32_QByteArray>::default();
        roles.insert(257, cxx_qt_lib::QByteArray::from("conversationId"));
        roles.insert(258, cxx_qt_lib::QByteArray::from("title"));
        roles.insert(259, cxx_qt_lib::QByteArray::from("updatedAt"));
        roles.insert(260, cxx_qt_lib::QByteArray::from("model"));
        roles.insert(261, cxx_qt_lib::QByteArray::from("messageCount"));
        roles
    }
}
