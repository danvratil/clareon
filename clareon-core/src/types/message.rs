// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Message types

use serde::{Deserialize, Serialize};

use super::{ContentBlock, ConversationId};

/// Role of the message sender
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Message from the user
    User,
    /// Message from the assistant
    Assistant,
}

impl Role {
    /// Returns the string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            other => Err(format!("Invalid role: {}", other)),
        }
    }
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier (database primary key)
    pub id: i64,

    /// ID of the conversation this message belongs to
    pub conversation_id: ConversationId,

    /// Unix timestamp when the message was created
    pub created_at: i64,

    /// Role of the message sender
    pub role: Role,

    /// Plain text content (for search/display, may be None for tool-only messages)
    pub text_content: Option<String>,

    /// Full structured content blocks
    pub content: Vec<ContentBlock>,

    /// Number of input tokens (for assistant messages, the prompt tokens)
    pub input_tokens: Option<i64>,

    /// Number of output tokens (for assistant messages)
    pub output_tokens: Option<i64>,

    /// Model used to generate this message (for assistant messages)
    pub model: Option<String>,
}

impl Message {
    /// Create a new user message
    pub fn user(conversation_id: impl Into<ConversationId>, text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            id: 0, // Will be set by database
            conversation_id: conversation_id.into(),
            created_at: chrono::Utc::now().timestamp(),
            role: Role::User,
            text_content: Some(text.clone()),
            content: vec![ContentBlock::text(text)],
            input_tokens: None,
            output_tokens: None,
            model: None,
        }
    }

    /// Create a new user message with custom content blocks
    pub fn user_with_content(
        conversation_id: impl Into<ConversationId>,
        content: Vec<ContentBlock>,
    ) -> Self {
        // Extract text content for FTS indexing
        let text_content = Self::extract_text(&content);

        Self {
            id: 0, // Will be set by database
            conversation_id: conversation_id.into(),
            created_at: chrono::Utc::now().timestamp(),
            role: Role::User,
            text_content,
            content,
            input_tokens: None,
            output_tokens: None,
            model: None,
        }
    }

    /// Create a new assistant message
    pub fn assistant(
        conversation_id: impl Into<ConversationId>,
        content: Vec<ContentBlock>,
        model: impl Into<String>,
        input_tokens: i64,
        output_tokens: i64,
    ) -> Self {
        // Extract text content for FTS indexing
        let text_content = Self::extract_text(&content);

        Self {
            id: 0, // Will be set by database
            conversation_id: conversation_id.into(),
            created_at: chrono::Utc::now().timestamp(),
            role: Role::Assistant,
            text_content,
            content,
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            model: Some(model.into()),
        }
    }

    /// Extract text content from content blocks for FTS indexing
    fn extract_text(content: &[ContentBlock]) -> Option<String> {
        let texts: Vec<&str> = content.iter().filter_map(|block| block.as_text()).collect();

        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    }

    /// Check if this message contains any tool use requests
    pub fn has_tool_use(&self) -> bool {
        self.content.iter().any(|block| block.is_tool_use())
    }

    /// Get all tool use blocks from this message
    pub fn tool_uses(&self) -> impl Iterator<Item = &ContentBlock> {
        self.content.iter().filter(|block| block.is_tool_use())
    }

    /// Get the plain text content of this message
    pub fn text(&self) -> Option<&str> {
        self.text_content.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn test_role_parsing() {
        assert_eq!("user".parse::<Role>().unwrap(), Role::User);
        assert_eq!("assistant".parse::<Role>().unwrap(), Role::Assistant);
        assert_eq!("USER".parse::<Role>().unwrap(), Role::User);
    }

    #[test]
    fn test_user_message_creation() {
        let msg = Message::user("test-conversation-id", "Hello!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.text_content, Some("Hello!".to_string()));
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn test_extract_text() {
        let content = vec![
            ContentBlock::text("First part"),
            ContentBlock::tool_use("123", "test", serde_json::json!({})),
            ContentBlock::text("Second part"),
        ];
        let text = Message::extract_text(&content);
        assert_eq!(text, Some("First part\nSecond part".to_string()));
    }
}
