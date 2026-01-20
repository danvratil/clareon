// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! MessageListModel - Qt model for messages in a conversation

use chrono::DateTime;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QList, QModelIndex, QString, QVariant,
};
use tokio::task::JoinHandle;

use clareon_core::types::ConversationId;

use crate::service::{Command, ErrorInfo, MessageData, Response};
use crate::service_controller::try_get_service_handle;

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("QtCore/QAbstractListModel");
        type QAbstractListModel;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, conversation_id, READ, WRITE = set_conversation_id, NOTIFY)]
        type MessageListModel = super::MessageListModelRust;

        fn set_conversation_id(self: Pin<&mut MessageListModel>, id: QString);

        #[cxx_override]
        fn row_count(self: &MessageListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &MessageListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        fn role_names(self: &MessageListModel) -> QHash_i32_QByteArray;

        #[inherit]
        fn index(
            self: &MessageListModel,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;

        #[inherit]
        fn begin_insert_rows(
            self: Pin<&mut MessageListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        fn end_insert_rows(self: Pin<&mut MessageListModel>);

        #[inherit]
        fn begin_remove_rows(
            self: Pin<&mut MessageListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );

        #[inherit]
        fn end_remove_rows(self: Pin<&mut MessageListModel>);

        #[inherit]
        fn begin_reset_model(self: Pin<&mut MessageListModel>);

        #[inherit]
        fn end_reset_model(self: Pin<&mut MessageListModel>);

        #[inherit]
        fn data_changed(
            self: Pin<&mut MessageListModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: QList_i32,
        );
    }

    impl cxx_qt::Threading for MessageListModel {}
}

/// Custom roles for the model
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
enum MessageRole {
    MessageId = 0x1000 + 1, // Qt::UserRole + 1
    Role,
    Text,
    CreatedAt,
    MessageState,
    // Error-related roles
    IsError,
    ErrorMessage,
    ErrorDetails,
    ErrorCategory,
    IsRetryable,
    RetryAfterSecs,
    PartialContent,
}

#[derive(Default, Debug)]
pub enum MessageState {
    Thinking,
    Streaming,
    #[default]
    Complete,
    Error,
}

impl From<MessageRole> for i32 {
    fn from(role: MessageRole) -> Self {
        role as i32
    }
}

#[derive(Debug)]
struct Message {
    id: i64,
    role: String,
    text: String,
    created_at: i64,
    state: MessageState,
    // Error-related fields
    error_info: Option<ErrorInfo>,
    partial_content: Option<String>,
}

impl Message {
    fn from_message_data_with_state(data: MessageData, state: MessageState) -> Self {
        Self {
            id: data.id,
            role: data.role,
            text: data.text,
            created_at: data.created_at,
            state,
            error_info: None,
            partial_content: None,
        }
    }

    fn from_error(error_info: ErrorInfo, partial_content: Option<String>) -> Self {
        Self {
            id: -2, // Special ID for error messages
            role: "error".to_string(),
            text: error_info.message.clone(),
            created_at: chrono::Local::now().timestamp(),
            state: MessageState::Error,
            error_info: Some(error_info),
            partial_content,
        }
    }
}

impl From<MessageData> for Message {
    fn from(data: MessageData) -> Self {
        Self::from_message_data_with_state(data, MessageState::default())
    }
}

/// Rust implementation of MessageListModel
pub struct MessageListModelRust {
    conversation_id: QString,
    messages: Vec<Message>,
    subscription: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Default for MessageListModelRust {
    fn default() -> Self {
        Self {
            conversation_id: QString::default(),
            messages: Vec::new(),
            subscription: Arc::new(Mutex::new(None)),
        }
    }
}

#[allow(dead_code)]
impl ffi::MessageListModel {
    /// Set messages (replaces all existing messages)
    pub fn set_messages_internal(mut self: Pin<&mut Self>, messages: Vec<MessageData>) {
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().messages = messages.into_iter().map(Message::from).collect();
        self.as_mut().end_reset_model();
    }

    /// Add a single message to the end
    pub fn add_message_internal(mut self: Pin<&mut Self>, message: MessageData) {
        let count = self.rust().messages.len();
        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), count as i32, count as i32);
        self.as_mut()
            .rust_mut()
            .messages
            .push(Message::from(message));
        self.as_mut().end_insert_rows();
    }

    /// Clear all messages
    pub fn clear_internal(mut self: Pin<&mut Self>) {
        if self.rust().messages.is_empty() {
            return;
        }
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().messages.clear();
        self.as_mut().end_reset_model();
    }

    fn set_conversation_id(mut self: Pin<&mut Self>, id: QString) {
        if id == *self.conversation_id() {
            return;
        }

        self.as_mut().unsubscribe_from_events();
        self.as_mut().clear_internal();
        if id.is_empty() {
            return;
        }

        self.as_mut().rust_mut().conversation_id = id;
        self.as_mut().conversation_id_changed();

        self.as_mut().subscribe_to_events();

        if let Some(handle) = try_get_service_handle() {
            let conv_id = ConversationId::from(self.conversation_id().to_string());
            info!(
                conversation_id = self.conversation_id().to_string(),
                "Loading messages for new conversation in MessageListModel"
            );
            let _ = handle.send(Command::LoadMessages { conv_id });
        }
    }

    fn start_streaming_message(mut self: Pin<&mut Self>) {
        // Add a placeholder message for streaming
        let count = self.rust().messages.len();
        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), count as i32, count as i32);
        self.as_mut().rust_mut().messages.push(Message {
            id: -1,
            role: "assistant".to_string(),
            text: String::new(),
            created_at: DateTime::<chrono::Local>::default().timestamp(),
            state: MessageState::Thinking,
            error_info: None,
            partial_content: None,
        });
        self.as_mut().end_insert_rows();
    }

    fn update_streaming_message(mut self: Pin<&mut Self>, delta: String) {
        if let Some(message) = self.as_mut().rust_mut().messages.last_mut() {
            assert!(message.id == -1); // Ensure it's the streaming placeholder
            message.text.push_str(&delta);
            message.state = MessageState::Streaming;
            let row = (self.rust().messages.len() - 1) as i32;
            let index = self.as_ref().index(row, 0, &QModelIndex::default());
            self.as_mut().data_changed(
                &index,
                &index,
                QList::from(&[MessageRole::Text.into(), MessageRole::MessageState.into()]),
            );
        }
    }

    fn complete_streaming_message(mut self: Pin<&mut Self>, message: MessageData) {
        if let Some(last_message) = self.as_mut().rust_mut().messages.last_mut() {
            assert!(last_message.id == -1); // Ensure it's the streaming placeholder
            *last_message = message.into();
            let row = (self.rust().messages.len() - 1) as i32;
            let index = self.as_ref().index(row, 0, &QModelIndex::default());
            self.as_mut().data_changed(
                &index,
                &index,
                QList::from(&[
                    MessageRole::MessageId.into(),
                    MessageRole::Role.into(),
                    MessageRole::Text.into(),
                    MessageRole::CreatedAt.into(),
                    MessageRole::MessageState.into(),
                ]),
            );
        }
    }

    fn add_error_message(mut self: Pin<&mut Self>, error_info: ErrorInfo, partial: Option<String>) {
        let count = self.rust().messages.len();
        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), count as i32, count as i32);
        self.as_mut()
            .rust_mut()
            .messages
            .push(Message::from_error(error_info, partial));
        self.as_mut().end_insert_rows();
    }

    fn replace_streaming_with_error(
        mut self: Pin<&mut Self>,
        error_info: ErrorInfo,
        partial: Option<String>,
    ) {
        if let Some(last_message) = self.as_mut().rust_mut().messages.last_mut()
            && last_message.id == -1
        {
            // Replace streaming placeholder with error
            *last_message = Message::from_error(error_info, partial);
            let row = (self.rust().messages.len() - 1) as i32;
            let index = self.as_ref().index(row, 0, &QModelIndex::default());
            // Notify all roles changed
            self.as_mut().data_changed(&index, &index, QList::default());
        }
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().messages.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row();
        if row < 0 || row as usize >= self.rust().messages.len() {
            return QVariant::default();
        }

        let message = &self.rust().messages[row as usize];

        if role == MessageRole::MessageId as i32 {
            QVariant::from(&message.id)
        } else if role == MessageRole::Role as i32 {
            QVariant::from(&QString::from(&message.role))
        } else if role == MessageRole::Text as i32 {
            QVariant::from(&QString::from(&message.text))
        } else if role == MessageRole::CreatedAt as i32 {
            QVariant::from(&message.created_at)
        } else if role == MessageRole::MessageState as i32 {
            let state_str = match message.state {
                MessageState::Thinking => "thinking",
                MessageState::Streaming => "streaming",
                MessageState::Complete => "complete",
                MessageState::Error => "error",
            };
            QVariant::from(&QString::from(state_str))
        } else if role == MessageRole::IsError as i32 {
            QVariant::from(&message.error_info.is_some())
        } else if role == MessageRole::ErrorMessage as i32 {
            if let Some(ref info) = message.error_info {
                QVariant::from(&QString::from(&info.message))
            } else {
                QVariant::default()
            }
        } else if role == MessageRole::ErrorDetails as i32 {
            if let Some(ref info) = message.error_info {
                QVariant::from(&QString::from(&info.details))
            } else {
                QVariant::default()
            }
        } else if role == MessageRole::ErrorCategory as i32 {
            if let Some(ref info) = message.error_info {
                let category_str = match info.category {
                    crate::service::ErrorCategory::Network => "network",
                    crate::service::ErrorCategory::RateLimit => "ratelimit",
                    crate::service::ErrorCategory::Authentication => "authentication",
                    crate::service::ErrorCategory::ServerError => "servererror",
                    crate::service::ErrorCategory::ClientError => "clienterror",
                    crate::service::ErrorCategory::ContextLimit => "contextlimit",
                    crate::service::ErrorCategory::Unknown => "unknown",
                };
                QVariant::from(&QString::from(category_str))
            } else {
                QVariant::default()
            }
        } else if role == MessageRole::IsRetryable as i32 {
            if let Some(ref info) = message.error_info {
                QVariant::from(&info.is_retryable)
            } else {
                QVariant::from(&false)
            }
        } else if role == MessageRole::RetryAfterSecs as i32 {
            if let Some(ref info) = message.error_info {
                QVariant::from(&(info.retry_after_secs.unwrap_or(0) as i32))
            } else {
                QVariant::from(&0i32)
            }
        } else if role == MessageRole::PartialContent as i32 {
            if let Some(ref partial) = message.partial_content {
                QVariant::from(&QString::from(partial))
            } else {
                QVariant::default()
            }
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::default();
        roles.insert(MessageRole::MessageId.into(), QByteArray::from("messageId"));
        roles.insert(MessageRole::Role.into(), QByteArray::from("role"));
        roles.insert(MessageRole::Text.into(), QByteArray::from("textContent"));
        roles.insert(MessageRole::CreatedAt.into(), QByteArray::from("createdAt"));
        roles.insert(
            MessageRole::MessageState.into(),
            QByteArray::from("messageState"),
        );
        // Error-related roles
        roles.insert(MessageRole::IsError.into(), QByteArray::from("isError"));
        roles.insert(
            MessageRole::ErrorMessage.into(),
            QByteArray::from("errorMessage"),
        );
        roles.insert(
            MessageRole::ErrorDetails.into(),
            QByteArray::from("errorDetails"),
        );
        roles.insert(
            MessageRole::ErrorCategory.into(),
            QByteArray::from("errorCategory"),
        );
        roles.insert(
            MessageRole::IsRetryable.into(),
            QByteArray::from("isRetryable"),
        );
        roles.insert(
            MessageRole::RetryAfterSecs.into(),
            QByteArray::from("retryAfterSecs"),
        );
        roles.insert(
            MessageRole::PartialContent.into(),
            QByteArray::from("partialContent"),
        );
        roles
    }

    /// Subscribe to broadcast events for the current conversation
    fn subscribe_to_events(self: Pin<&mut Self>) {
        let conv_id_str = self.conversation_id().to_string();
        debug!(
            conversation_id = conv_id_str,
            "Subscribing to conversation events for MessageListModel"
        );
        if conv_id_str.is_empty() {
            return;
        }

        let conv_id = ConversationId::from(conv_id_str);
        let qt_thread = self.qt_thread();

        // Get service handle and subscribe
        let handle = match crate::service_controller::try_get_service_handle() {
            Some(h) => h,
            None => return,
        };

        let mut response_rx = handle.subscribe();

        // Spawn async task to receive and filter events
        let task = crate::get_runtime().spawn(async move {
            while let Ok(response) = response_rx.recv().await {
                // Filter by conversation_id
                let is_relevant = match &response {
                    Response::MessagesLoaded { conv_id: id, .. } => *id == conv_id,
                    Response::MessageSent { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingStarted { conv_id: id } => *id == conv_id,
                    Response::StreamingChunk { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingComplete { conv_id: id, .. } => *id == conv_id,
                    Response::SendMessageError { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingError { conv_id: id, .. } => *id == conv_id,
                    _ => false,
                };

                if !is_relevant {
                    continue;
                }

                // Queue update to Qt thread
                let _ = qt_thread.queue(move |mut model| {
                    model.as_mut().handle_response(response);
                });
            }
        });

        // Store the task handle
        *self
            .rust()
            .subscription
            .lock()
            .expect("Subscription lock poisoned") = Some(task);
    }

    /// Unsubscribe from broadcast events
    fn unsubscribe_from_events(self: Pin<&mut Self>) {
        // Abort the subscription task
        if let Some(task) = self
            .rust()
            .subscription
            .lock()
            .expect("Subscription lock poisoned")
            .take()
        {
            debug!(
                conversation_id = self.conversation_id().to_string(),
                "Unsubscribing from conversation events for MessageListModel"
            );
            task.abort();
        }
    }

    /// Handle a filtered response
    fn handle_response(mut self: Pin<&mut Self>, response: crate::service::Response) {
        match response {
            Response::MessagesLoaded { messages, .. } => {
                self.as_mut().set_messages_internal(messages);
            }
            Response::MessageSent {
                conv_id: _,
                message,
            } => {
                self.as_mut().add_message_internal(message);
            }
            Response::StreamingStarted { conv_id: _ } => {
                self.as_mut().start_streaming_message();
            }
            Response::StreamingChunk {
                conv_id: _,
                delta,
                accumulated: _,
            } => {
                self.as_mut().update_streaming_message(delta);
            }
            Response::StreamingComplete { message, .. } => {
                self.as_mut().complete_streaming_message(message);
            }
            Response::SendMessageError {
                error_info,
                user_message_id: _,
                ..
            } => {
                // Remove any temporary messages (optimistic user message with id -2, or thinking placeholder with id -1)
                if let Some(last) = self.rust().messages.last()
                    && (last.id == -1 || last.id == -2)
                {
                    let count = self.rust().messages.len();
                    self.as_mut().begin_remove_rows(
                        &QModelIndex::default(),
                        (count - 1) as i32,
                        (count - 1) as i32,
                    );
                    self.as_mut().rust_mut().messages.pop();
                    self.as_mut().end_remove_rows();
                }
                // Add error message
                self.as_mut().add_error_message(error_info, None);
            }
            Response::StreamingError {
                error_info,
                partial_text,
                ..
            } => {
                // Replace streaming message with error showing partial content
                self.as_mut()
                    .replace_streaming_with_error(error_info, Some(partial_text));
            }
            _ => {
                // Ignore other response types
            }
        }
    }
}

impl Drop for MessageListModelRust {
    fn drop(&mut self) {
        // Abort the subscription task when model is destroyed
        if let Some(task) = self
            .subscription
            .lock()
            .expect("Subscription lock poisoned")
            .take()
        {
            task.abort();
        }
    }
}
