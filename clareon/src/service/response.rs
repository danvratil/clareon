// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Responses sent from the service worker to the Qt layer

use clareon_core::types::{Conversation, ConversationId, ConversationSummary};

/// Simplified message data for Qt consumption
#[derive(Debug, Clone)]
pub struct MessageData {
    pub id: i64,
    pub role: String, // "user" or "assistant"
    pub text: String,
    pub created_at: i64,
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

    // Error response
    /// An error occurred while processing a command
    Error { command: String, error: String },
}
