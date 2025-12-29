// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use clareon_core::ConversationManager;
use std::sync::{Arc, OnceLock};

static RUNTIME: OnceLock<Arc<tokio::runtime::Runtime>> = OnceLock::new();
static MANAGER: OnceLock<Arc<ConversationManager>> = OnceLock::new();

pub fn init_runtime(runtime: Arc<tokio::runtime::Runtime>, manager: Arc<ConversationManager>) {
    RUNTIME.set(runtime).ok();
    MANAGER.set(manager).ok();
}

fn get_runtime() -> Arc<tokio::runtime::Runtime> {
    RUNTIME.get().expect("Runtime not initialized").clone()
}

fn get_manager() -> Arc<ConversationManager> {
    MANAGER.get().expect("Manager not initialized").clone()
}

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

        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut MessageListModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut MessageListModel>);

        #[qinvokable]
        #[cxx_name = "appendMessage"]
        fn append_message(self: Pin<&mut MessageListModel>, role: &QString, text: &QString);

        #[qinvokable]
        #[cxx_name = "loadMessages"]
        fn load_messages(self: Pin<&mut MessageListModel>, conversation_id: &QString);
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

        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut ConversationListModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut ConversationListModel>);

        #[qinvokable]
        #[cxx_name = "refreshConversations"]
        fn refresh_conversations(self: Pin<&mut ConversationListModel>);
    }
}

use chrono::Utc;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};
use std::pin::Pin;

// Real conversation row for the model
struct ConversationRow {
    id: String, // UUID
    title: String,
    updated_at: i64,
    model: String,
    message_count: i64,
}

// Real message row for the model
struct MessageRow {
    id: i64,
    role: String,
    text_content: String,
    created_at: i64,
}

#[derive(Default)]
pub struct MessageListModelRust {
    messages: Vec<MessageRow>,
}

impl ffi::MessageListModel {
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.messages.len() as i32
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row();

        if row < 0 || row >= self.messages.len() as i32 {
            return QVariant::default();
        }

        let message = &self.messages[row as usize];

        match role {
            257 => QVariant::from(&(message.id as i32)),
            258 => QVariant::from(&QString::from(&message.role)),
            259 => QVariant::from(&QString::from(&message.text_content)),
            260 => QVariant::from(&(message.created_at as i32)),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(257, QByteArray::from("messageId"));
        roles.insert(258, QByteArray::from("role"));
        roles.insert(259, QByteArray::from("textContent"));
        roles.insert(260, QByteArray::from("createdAt"));
        roles
    }

    pub fn append_message(mut self: Pin<&mut Self>, role: &QString, text: &QString) {
        let role_str = role.to_string();
        let text_str = text.to_string();

        let new_id = self.messages.last().map(|m| m.id + 1).unwrap_or(1);
        let created_at = Utc::now().timestamp();

        let new_message = MessageRow {
            id: new_id,
            role: role_str,
            text_content: text_str,
            created_at,
        };

        let row = self.messages.len() as i32;

        unsafe {
            self.as_mut()
                .begin_insert_rows(&QModelIndex::default(), row, row);
        }
        self.as_mut().rust_mut().messages.push(new_message);
        unsafe {
            self.as_mut().end_insert_rows();
        }
    }

    pub fn load_messages(mut self: Pin<&mut Self>, conversation_id: &QString) {
        use clareon_core::types::ConversationId;

        let conv_id = ConversationId::from(conversation_id.to_string());
        let runtime = get_runtime();
        let manager = get_manager();

        // Load messages from database (blocks UI briefly)
        let messages =
            runtime.block_on(async { manager.get_messages(&conv_id).await.unwrap_or_default() });

        // Convert to Qt rows
        let rows: Vec<MessageRow> = messages
            .into_iter()
            .map(|m| MessageRow {
                id: m.id,
                role: m.role.as_str().to_string(),
                text_content: m.text().unwrap_or_default().to_string(),
                created_at: m.created_at,
            })
            .collect();

        // Update model
        unsafe {
            self.as_mut().begin_reset_model();
        }
        self.as_mut().rust_mut().messages = rows;
        unsafe {
            self.as_mut().end_reset_model();
        }
    }
}

#[derive(Default)]
pub struct ConversationListModelRust {
    conversations: Vec<ConversationRow>,
}

impl ffi::ConversationListModel {
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.conversations.len() as i32
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row();

        if row < 0 || row >= self.conversations.len() as i32 {
            return QVariant::default();
        }

        let conversation = &self.conversations[row as usize];

        match role {
            257 => QVariant::from(&QString::from(&conversation.id)),
            258 => QVariant::from(&QString::from(&conversation.title)),
            259 => QVariant::from(&(conversation.updated_at as i32)),
            260 => QVariant::from(&QString::from(&conversation.model)),
            261 => QVariant::from(&(conversation.message_count as i32)),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(257, QByteArray::from("conversationId"));
        roles.insert(258, QByteArray::from("title"));
        roles.insert(259, QByteArray::from("updatedAt"));
        roles.insert(260, QByteArray::from("model"));
        roles.insert(261, QByteArray::from("messageCount"));
        roles
    }

    pub fn refresh_conversations(mut self: Pin<&mut Self>) {
        let runtime = get_runtime();
        let manager = get_manager();

        // Load conversations from database (blocks UI briefly)
        let conversations =
            runtime.block_on(async { manager.list_conversations().await.unwrap_or_default() });

        // Convert to Qt rows
        let rows: Vec<ConversationRow> = conversations
            .into_iter()
            .map(|c| ConversationRow {
                id: c.id.to_string(),
                title: c.title.unwrap_or_else(|| "Untitled".to_string()),
                updated_at: c.updated_at,
                model: c.model,
                message_count: c.message_count,
            })
            .collect();

        // Update model
        unsafe {
            self.as_mut().begin_reset_model();
        }
        self.as_mut().rust_mut().conversations = rows;
        unsafe {
            self.as_mut().end_reset_model();
        }
    }
}
