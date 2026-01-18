// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Responses sent from the service worker to the Qt layer

use clareon_core::types::{Conversation, ConversationId, ConversationSummary, SearchResult};

/// Simplified message data for Qt consumption
#[derive(Debug, Clone)]
pub struct MessageData {
    pub id: i64,
    pub role: String, // "user" or "assistant"
    pub text: String,
    pub created_at: i64,
}

/// Information about an error that can be displayed to the user
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    /// User-friendly error message
    pub message: String,

    /// Technical error details (for advanced users/debugging)
    pub details: String,

    /// Error category
    pub category: ErrorCategory,

    /// Whether this error can be retried
    pub is_retryable: bool,

    /// Optional retry delay in seconds (for rate limiting)
    pub retry_after_secs: Option<u64>,
}

/// Error category for classification and display
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Network errors (connection failures, timeouts)
    Network,
    /// Rate limited by API
    RateLimit,
    /// Invalid API key/credentials
    Authentication,
    /// Service unavailable, 5xx errors
    ServerError,
    /// Invalid request, model not available
    ClientError,
    /// Context length exceeded
    ContextLimit,
    /// Other/unknown errors
    Unknown,
}

/// Responses represent all events and results sent from the service to the UI
#[derive(Debug, Clone)]
pub enum Response {
    // Conversation responses
    /// A new conversation was created
    ConversationCreated { conversation: ConversationSummary },

    /// A conversation was loaded
    ConversationLoaded { conversation: Conversation },

    /// A conversation was deleted
    ConversationDeleted { id: ConversationId },

    /// The list of conversations was refreshed
    ConversationsRefreshed {
        conversations: Vec<ConversationSummary>,
    },

    // Message responses
    /// Messages for a conversation were loaded
    MessagesLoaded {
        conv_id: ConversationId,
        messages: Vec<MessageData>,
    },

    /// A message was sent (final, after streaming completes)
    MessageSent {
        conv_id: ConversationId,
        message: MessageData,
    },

    // Streaming responses
    /// Streaming has started for a message
    StreamingStarted { conv_id: ConversationId },

    /// A chunk of streaming text arrived
    StreamingChunk {
        conv_id: ConversationId,
        delta: String,
        accumulated: String,
    },

    /// Streaming has completed
    StreamingComplete {
        conv_id: ConversationId,
        message: MessageData,
    },

    // Search responses
    /// Search results from FTS query
    SearchResults { results: Vec<SearchResult> },

    // Error responses
    /// Error occurred while sending a message (before streaming starts)
    SendMessageError {
        conv_id: ConversationId,
        error_info: ErrorInfo,
        /// ID of the user message that failed (if it was saved)
        user_message_id: Option<i64>,
    },

    /// Error occurred during message streaming
    StreamingError {
        conv_id: ConversationId,
        error_info: ErrorInfo,
        /// Partial content that was received before error
        partial_text: String,
    },

    /// Generic error for non-conversation operations
    Error { command: String, error: String },


    /// Main window activation was requested
    ActivateMainWindow,
}
