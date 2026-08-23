// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! MessageListModel - Qt model for messages in a conversation

use chrono::DateTime;
use std::pin::Pin;
use tracing::{debug, info};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QHash, QHashPair_i32_QByteArray, QList, QModelIndex, QString, QVariant};

use clareon_core::types::ConversationId;

use crate::model_helpers::{Subscription, get_item, make_role_names};
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
        #[qproperty(i64, total_input_tokens, READ, NOTIFY)]
        #[qproperty(i64, total_output_tokens, READ, NOTIFY)]
        #[qproperty(bool, streaming, READ, NOTIFY)]
        #[qproperty(QString, pending_approval_json, READ, NOTIFY)]
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
    ContentBlocks,         // new
    IsGroupedWithPrevious, // new
}

#[derive(Default, Debug)]
pub enum MessageState {
    Thinking,
    Streaming,
    #[default]
    Complete,
    Error,
}

/// Maximum time gap (seconds) between consecutive same-role messages
/// for them to be considered visually grouped.
const GROUPING_GAP_SECS: i64 = 300;

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
    // Token usage fields
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    is_grouped_with_previous: bool,
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
            input_tokens: data.input_tokens,
            output_tokens: data.output_tokens,
            is_grouped_with_previous: false,
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
            input_tokens: None,
            output_tokens: None,
            is_grouped_with_previous: false,
        }
    }
}

impl From<MessageData> for Message {
    fn from(data: MessageData) -> Self {
        Self::from_message_data_with_state(data, MessageState::default())
    }
}

/// Rust implementation of MessageListModel
#[derive(Default)]
pub struct MessageListModelRust {
    conversation_id: QString,
    messages: Vec<Message>,
    subscription: Subscription,
    total_input_tokens: i64,
    total_output_tokens: i64,
    streaming: bool,
    pending_approval_json: QString,
}

/// Computes `is_grouped_with_previous` for every message in the slice.
/// A message is grouped when it has the same role as its predecessor and
/// the time gap is ≤ 300 seconds (5 minutes).
fn compute_grouping(messages: &mut [Message]) {
    for i in 1..messages.len() {
        let same_role = messages[i - 1].role == messages[i].role;
        let gap = messages[i].created_at - messages[i - 1].created_at;
        messages[i].is_grouped_with_previous = same_role && gap <= GROUPING_GAP_SECS;
    }
}

/// Splits message text into typed content blocks serialized as a JSON array.
/// Each block is either `{"type":"text","content":"..."}` or
/// `{"type":"code","language":"...","content":"..."}`.
fn parse_content_blocks(text: &str) -> String {
    #[derive(serde::Serialize)]
    #[serde(tag = "type", rename_all = "lowercase")]
    enum Block<'a> {
        Text { content: &'a str },
        Code { language: &'a str, content: &'a str },
    }

    let mut blocks: Vec<Block> = Vec::new();
    let mut remaining = text;

    while let Some(fence_start) = remaining.find("```") {
        if fence_start > 0 {
            blocks.push(Block::Text {
                content: &remaining[..fence_start],
            });
        }
        remaining = &remaining[fence_start + 3..];

        let lang_end = remaining.find('\n').unwrap_or(remaining.len());
        let language = remaining[..lang_end].trim();
        remaining = if lang_end < remaining.len() {
            &remaining[lang_end + 1..]
        } else {
            ""
        };

        if let Some(close) = remaining.find("```") {
            blocks.push(Block::Code {
                language,
                content: &remaining[..close],
            });
            remaining = &remaining[close + 3..];
            if remaining.starts_with('\n') {
                remaining = &remaining[1..];
            }
        } else {
            blocks.push(Block::Code {
                language,
                content: remaining,
            });
            remaining = "";
        }
    }

    if !remaining.is_empty() {
        blocks.push(Block::Text { content: remaining });
    }

    if blocks.is_empty() {
        blocks.push(Block::Text { content: text });
    }

    serde_json::to_string(&blocks).unwrap_or_else(|_| "[]".to_string())
}

#[allow(dead_code)]
impl ffi::MessageListModel {
    /// Set messages (replaces all existing messages)
    pub fn set_messages_internal(mut self: Pin<&mut Self>, messages: Vec<MessageData>) {
        self.as_mut().begin_reset_model();
        let mut msgs: Vec<Message> = messages.into_iter().map(Message::from).collect();
        compute_grouping(&mut msgs);
        self.as_mut().rust_mut().messages = msgs;
        self.as_mut().end_reset_model();
        self.as_mut().recalculate_token_totals();
    }

    /// Add a single message to the end
    pub fn add_message_internal(mut self: Pin<&mut Self>, message: MessageData) {
        let count = self.rust().messages.len();
        self.as_mut()
            .begin_insert_rows(&QModelIndex::default(), count as i32, count as i32);
        let mut msg = Message::from(message);
        if let Some(prev) = self.rust().messages.last() {
            let same_role = prev.role == msg.role;
            let gap = msg.created_at - prev.created_at;
            msg.is_grouped_with_previous = same_role && gap <= GROUPING_GAP_SECS;
        }
        self.as_mut().add_to_token_totals(&msg);
        self.as_mut().rust_mut().messages.push(msg);
        self.as_mut().end_insert_rows();
    }

    /// Clear all messages
    pub fn clear_internal(mut self: Pin<&mut Self>) {
        if self.rust().messages.is_empty() {
            return;
        }
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().messages.clear();
        self.as_mut().reset_token_totals();
        self.as_mut().end_reset_model();
    }

    /// Recalculate token totals from all messages
    fn recalculate_token_totals(mut self: Pin<&mut Self>) {
        let (input_total, output_total) =
            self.rust()
                .messages
                .iter()
                .fold((0i64, 0i64), |(input_acc, output_acc), msg| {
                    (
                        input_acc + msg.input_tokens.unwrap_or(0),
                        output_acc + msg.output_tokens.unwrap_or(0),
                    )
                });

        self.as_mut().rust_mut().total_input_tokens = input_total;
        self.as_mut().rust_mut().total_output_tokens = output_total;
        self.as_mut().total_input_tokens_changed();
        self.as_mut().total_output_tokens_changed();
    }

    /// Reset token totals to zero
    fn reset_token_totals(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().total_input_tokens = 0;
        self.as_mut().rust_mut().total_output_tokens = 0;
        self.as_mut().total_input_tokens_changed();
        self.as_mut().total_output_tokens_changed();
    }

    /// Add token counts from a message to the totals
    fn add_to_token_totals(mut self: Pin<&mut Self>, message: &Message) {
        self.as_mut().rust_mut().total_input_tokens += message.input_tokens.unwrap_or(0);
        self.as_mut().rust_mut().total_output_tokens += message.output_tokens.unwrap_or(0);
        self.as_mut().total_input_tokens_changed();
        self.as_mut().total_output_tokens_changed();
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

    fn set_streaming_state(mut self: Pin<&mut Self>, streaming: bool) {
        if self.rust().streaming == streaming {
            return;
        }
        self.as_mut().rust_mut().streaming = streaming;
        self.as_mut().streaming_changed();
    }

    fn set_pending_approval(mut self: Pin<&mut Self>, tools_json: String) {
        self.as_mut().rust_mut().pending_approval_json = QString::from(&tools_json);
        self.as_mut().pending_approval_json_changed();
    }

    fn clear_pending_approval(mut self: Pin<&mut Self>) {
        if self.rust().pending_approval_json.is_empty() {
            return;
        }
        self.as_mut().rust_mut().pending_approval_json = QString::default();
        self.as_mut().pending_approval_json_changed();
    }

    fn remove_streaming_placeholder(mut self: Pin<&mut Self>) {
        if let Some(last) = self.rust().messages.last()
            && last.id == -1
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
            input_tokens: None,
            output_tokens: None,
            is_grouped_with_previous: false,
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
                QList::from(&[
                    MessageRole::Text.into(),
                    MessageRole::MessageState.into(),
                    MessageRole::ContentBlocks.into(),
                ]),
            );
        }
    }

    fn complete_streaming_message(mut self: Pin<&mut Self>, message: MessageData) {
        if self
            .rust()
            .messages
            .last()
            .map(|m| m.id == -1)
            .unwrap_or(false)
        {
            let mut completed_msg = Message::from(message);
            // Compute grouping against the message before the streaming placeholder
            let n = self.rust().messages.len();
            if n >= 2 {
                let prev = &self.rust().messages[n - 2];
                let same_role = prev.role == completed_msg.role;
                let gap = completed_msg.created_at - prev.created_at;
                completed_msg.is_grouped_with_previous = same_role && gap <= GROUPING_GAP_SECS;
            }
            self.as_mut().add_to_token_totals(&completed_msg);
            if let Some(last_message) = self.as_mut().rust_mut().messages.last_mut() {
                *last_message = completed_msg;
            }
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
                    MessageRole::IsGroupedWithPrevious.into(),
                    MessageRole::ContentBlocks.into(),
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
        // Remove the streaming placeholder (id == -1) if present, then append
        // a new error row via add_error_message. The remove+insert pair uses
        // structural model signals which reliably refresh Repeater delegate
        // bindings; an in-place mutation with data_changed(.., QList::default())
        // does not always propagate boolean role changes (e.g. isError) through
        // QML required properties, leaving the error invisible.
        if let Some(last) = self.rust().messages.last()
            && last.id == -1
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
        self.as_mut().add_error_message(error_info, partial);
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().messages.len() as i32
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(message) = get_item(&self.rust().messages, index) else {
            return QVariant::default();
        };

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
        } else if role == MessageRole::ContentBlocks as i32 {
            QVariant::from(&QString::from(parse_content_blocks(&message.text).as_str()))
        } else if role == MessageRole::IsGroupedWithPrevious as i32 {
            QVariant::from(&message.is_grouped_with_previous)
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        make_role_names(&[
            (MessageRole::MessageId.into(), "messageId"),
            (MessageRole::Role.into(), "role"),
            (MessageRole::Text.into(), "textContent"),
            (MessageRole::CreatedAt.into(), "createdAt"),
            (MessageRole::MessageState.into(), "messageState"),
            (MessageRole::IsError.into(), "isError"),
            (MessageRole::ErrorMessage.into(), "errorMessage"),
            (MessageRole::ErrorDetails.into(), "errorDetails"),
            (MessageRole::ErrorCategory.into(), "errorCategory"),
            (MessageRole::IsRetryable.into(), "isRetryable"),
            (MessageRole::RetryAfterSecs.into(), "retryAfterSecs"),
            (MessageRole::PartialContent.into(), "partialContent"),
            (MessageRole::ContentBlocks.into(), "contentBlocks"),
            (
                MessageRole::IsGroupedWithPrevious.into(),
                "isGroupedWithPrevious",
            ),
        ])
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
                let is_relevant = match &response {
                    Response::MessagesLoaded { conv_id: id, .. } => *id == conv_id,
                    Response::MessageSent { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingStarted { conv_id: id } => *id == conv_id,
                    Response::StreamingChunk { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingComplete { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingStopped { conv_id: id, .. } => *id == conv_id,
                    Response::ToolApprovalRequired { conv_id: id, .. } => *id == conv_id,
                    Response::SendMessageError { conv_id: id, .. } => *id == conv_id,
                    Response::StreamingError { conv_id: id, .. } => *id == conv_id,
                    _ => false,
                };

                if !is_relevant {
                    continue;
                }

                let _ = qt_thread.queue(move |mut model| {
                    model.as_mut().handle_response(response);
                });
            }
        });

        self.rust().subscription.start(task);
    }

    /// Unsubscribe from broadcast events
    fn unsubscribe_from_events(self: Pin<&mut Self>) {
        debug!(
            conversation_id = self.conversation_id().to_string(),
            "Unsubscribing from conversation events for MessageListModel"
        );
        self.rust().subscription.cancel();
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
                self.as_mut().set_streaming_state(true);
                self.as_mut().clear_pending_approval();
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
                self.as_mut().set_streaming_state(false);
                self.as_mut().clear_pending_approval();
                self.as_mut().complete_streaming_message(message);
            }
            Response::StreamingStopped { message, .. } => {
                self.as_mut().set_streaming_state(false);
                self.as_mut().clear_pending_approval();
                if let Some(message) = message {
                    self.as_mut().complete_streaming_message(message);
                } else {
                    self.as_mut().remove_streaming_placeholder();
                }
            }
            Response::ToolApprovalRequired { tools_json, .. } => {
                if tools_json.is_empty() || tools_json == "[]" {
                    self.as_mut().clear_pending_approval();
                } else {
                    self.as_mut().set_pending_approval(tools_json);
                }
            }
            Response::SendMessageError { error_info, .. } => {
                self.as_mut().set_streaming_state(false);
                self.as_mut().clear_pending_approval();
                // Remove "thinking" placeholder if present
                self.as_mut().remove_streaming_placeholder();
                // Add error message
                self.as_mut().add_error_message(error_info, None);
            }
            Response::StreamingError {
                error_info,
                partial_text,
                ..
            } => {
                self.as_mut().set_streaming_state(false);
                self.as_mut().clear_pending_approval();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let result = parse_content_blocks("Hello, world!");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let blocks = parsed.as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[0]["content"], "Hello, world!");
    }

    #[test]
    fn test_parse_single_code_block() {
        let input = "Before\n```python\nprint('hi')\n```\nAfter";
        let result = parse_content_blocks(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let blocks = parsed.as_array().unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "code");
        assert_eq!(blocks[1]["language"], "python");
        assert_eq!(blocks[1]["content"], "print('hi')\n");
        assert_eq!(blocks[2]["type"], "text");
        assert_eq!(blocks[2]["content"], "After");
    }

    #[test]
    fn test_parse_code_block_no_language() {
        let input = "```\nfoo()\n```";
        let result = parse_content_blocks(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let blocks = parsed.as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "code");
        assert_eq!(blocks[0]["language"], "");
    }

    #[test]
    fn test_parse_multiple_code_blocks() {
        let input = "Intro\n```rust\nfn main() {}\n```\nMiddle\n```python\npass\n```\nEnd";
        let result = parse_content_blocks(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let blocks = parsed.as_array().unwrap();
        assert_eq!(blocks.len(), 5);
        assert_eq!(blocks[0]["type"], "text");
        assert!(blocks[0]["content"].as_str().unwrap().contains("Intro"));
        assert_eq!(blocks[1]["type"], "code");
        assert_eq!(blocks[1]["language"], "rust");
        assert_eq!(blocks[2]["type"], "text");
        assert!(blocks[2]["content"].as_str().unwrap().contains("Middle"));
        assert_eq!(blocks[3]["type"], "code");
        assert_eq!(blocks[3]["language"], "python");
        assert_eq!(blocks[4]["type"], "text");
        assert!(blocks[4]["content"].as_str().unwrap().contains("End"));
    }

    #[test]
    fn test_parse_unclosed_fence() {
        // Simulates a streaming response where the code block hasn't closed yet
        let input = "Here is some code:\n```rust\nfn foo() {";
        let result = parse_content_blocks(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let blocks = parsed.as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "text");
        assert!(
            blocks[0]["content"]
                .as_str()
                .unwrap()
                .contains("Here is some code:")
        );
        assert_eq!(blocks[1]["type"], "code");
        assert_eq!(blocks[1]["language"], "rust");
        assert_eq!(blocks[1]["content"], "fn foo() {");
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_content_blocks("");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let blocks = parsed.as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
    }
}
