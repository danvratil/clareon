// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Commands sent from the Qt layer to the service worker

use clareon_core::config::Provider;
use clareon_core::types::{ContentBlock, ConversationId};

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

    /// Send a message with custom content blocks (e.g., with images)
    SendMessageWithContent {
        conv_id: ConversationId,
        content: Vec<ContentBlock>,
    },

    /// Send a message with attached files (stores files in database/workspace)
    SendMessageWithFiles {
        conv_id: ConversationId,
        text: String,
        file_paths: Vec<String>,
    },

    /// Load messages for a conversation
    LoadMessages { conv_id: ConversationId },

    /// Retry the last failed message in a conversation
    RetryLastMessage { conv_id: ConversationId },

    /// Search across all conversations
    Search { query: String },

    /// Create a new conversation and immediately send a message
    /// This is used for quick input flow
    NewQuickConversation { prompt: String },

    /// Load artifacts for a conversation
    LoadArtifacts { conv_id: ConversationId },

    /// Load a single artifact's content
    LoadArtifact { artifact_id: i64 },

    /// Save an artifact to a file
    SaveArtifact { artifact_id: i64, path: String },

    /// Reload configuration and recreate the backend
    ReloadConfig,

    /// Shutdown the service
    Shutdown,

    /// Activate main window
    ActivateMainWindow,

    /// Activate quick input window
    ActivateQuickInput,

    /// Fetch available models for a provider
    FetchAvailableModels { provider: Provider },

    /// List MCP server connection statuses
    ListMcpServers,

    /// List MCP resources (optional server filter)
    ListMcpResources { server_id: Option<String> },

    /// Read an MCP resource
    ReadMcpResource { server_id: String, uri: String },

    /// List MCP prompts (optional server filter)
    ListMcpPrompts { server_id: Option<String> },

    /// Get an MCP prompt template with optional arguments (JSON object string)
    GetMcpPrompt {
        server_id: String,
        name: String,
        /// JSON object of argument name → string/value
        arguments_json: String,
    },

    /// Inject an MCP prompt into the current conversation as a user message
    InjectMcpPrompt {
        conv_id: ConversationId,
        server_id: String,
        name: String,
        arguments_json: String,
    },

    /// Restart a single MCP server (reload all for simplicity)
    RestartMcpServers,

    /// Start interactive OAuth login for an MCP server (opens browser + localhost callback)
    StartMcpOAuthLogin { server_id: String },

    /// Clear stored OAuth tokens for an MCP server
    LogoutMcpOAuth { server_id: String },
}
