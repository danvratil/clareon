// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Commands sent from the Qt layer to the service worker

use clareon_core::types::ConversationId;

/// Commands represent all actions the UI can request from the service
#[derive(Debug, Clone)]
pub enum Command {
    /// Create a new conversation
    NewConversation,

    /// Load an existing conversation by ID
    LoadConversation { id: ConversationId },

    /// Delete a conversation
    DeleteConversation { id: ConversationId },

    /// Refresh the list of all conversations
    RefreshConversations,

    /// Send a message in a conversation
    SendMessage {
        conv_id: ConversationId,
        text: String,
    },

    /// Load messages for a conversation
    LoadMessages { conv_id: ConversationId },

    /// Retry the last failed message in a conversation
    RetryLastMessage { conv_id: ConversationId },

    /// Shutdown the service
    Shutdown,
}
