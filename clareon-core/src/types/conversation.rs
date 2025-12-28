// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conversation type

use serde::{Deserialize, Serialize};

/// A conversation with Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique identifier (database primary key)
    pub id: i64,

    /// Title of the conversation (auto-generated or user-provided)
    pub title: Option<String>,

    /// Unix timestamp when the conversation was created
    pub created_at: i64,

    /// Unix timestamp when the conversation was last updated
    pub updated_at: i64,

    /// Model used for this conversation
    pub model: String,

    /// Custom system prompt (None = use default)
    pub system_prompt: Option<String>,

    /// Additional instructions appended to the system prompt
    pub custom_instructions: Option<String>,
}

impl Conversation {
    /// Create a new conversation
    pub fn new(model: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: 0, // Will be set by database
            title: None,
            created_at: now,
            updated_at: now,
            model: model.into(),
            system_prompt: None,
            custom_instructions: None,
        }
    }

    /// Create a new conversation with a custom system prompt
    pub fn with_system_prompt(model: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        let mut conv = Self::new(model);
        conv.system_prompt = Some(system_prompt.into());
        conv
    }

    /// Set the title of the conversation
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Set custom instructions
    pub fn set_custom_instructions(&mut self, instructions: impl Into<String>) {
        self.custom_instructions = Some(instructions.into());
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Update the last modified timestamp
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Get a display title, falling back to "Untitled" if none set
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or("Untitled")
    }
}

/// Summary of a conversation for list display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    /// Conversation ID
    pub id: i64,

    /// Title
    pub title: Option<String>,

    /// When it was last updated
    pub updated_at: i64,

    /// Model used
    pub model: String,

    /// Number of messages in the conversation
    pub message_count: i64,
}

impl ConversationSummary {
    /// Get a display title, falling back to "Untitled" if none set
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or("Untitled")
    }
}

/// Search result from FTS query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Conversation ID
    pub conversation_id: i64,

    /// Conversation title
    pub conversation_title: Option<String>,

    /// Message ID that matched
    pub message_id: i64,

    /// Role of the message sender
    pub role: String,

    /// Matched text snippet (with highlighting)
    pub snippet: String,

    /// When the message was created
    pub created_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_creation() {
        let conv = Conversation::new("claude-sonnet-4-20250514");
        assert_eq!(conv.id, 0);
        assert_eq!(conv.title, None);
        assert_eq!(conv.model, "claude-sonnet-4-20250514");
        assert!(conv.created_at > 0);
        assert_eq!(conv.created_at, conv.updated_at);
    }

    #[test]
    fn test_display_title() {
        let mut conv = Conversation::new("test");
        assert_eq!(conv.display_title(), "Untitled");

        conv.set_title("My Chat");
        assert_eq!(conv.display_title(), "My Chat");
    }
}
