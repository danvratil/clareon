// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Title generation using a fast model (Haiku)

use std::sync::Arc;

use tracing::debug;

use crate::backend::{ChatRequest, LlmBackend};
use crate::error::{BackendError, Result};
use crate::types::Message;

const TITLE_SYSTEM_PROMPT: &str = r#"You are a title generator. Given a conversation between a user and an assistant, generate a short, descriptive title (3-6 words) that captures the main topic.

Rules:
- Output ONLY the title, nothing else
- No quotes, no punctuation at the end
- Be specific and descriptive
- Use sentence case (capitalize first word only, unless proper noun)"#;

/// Generates titles for conversations using a fast model
pub struct TitleGenerator {
    backend: Arc<dyn LlmBackend>,
    model: String,
}

impl TitleGenerator {
    /// Create a new title generator
    pub fn new(backend: Arc<dyn LlmBackend>, model: String) -> Self {
        Self { backend, model }
    }

    /// Generate a title for a conversation based on the first exchange
    pub async fn generate_title(
        &self,
        user_message: &str,
        assistant_response: &str,
    ) -> Result<String> {
        debug!("Generating title for conversation");

        // Truncate long messages to avoid using too many tokens
        let user_truncated = truncate(user_message, 500);
        let assistant_truncated = truncate(assistant_response, 500);

        let prompt = format!(
            "User: {}\n\nAssistant: {}\n\nGenerate a title for this conversation:",
            user_truncated, assistant_truncated
        );

        // Create a temporary message for the request (using placeholder conversation ID)
        let message = Message::user("temp", prompt);

        let request = ChatRequest::new(vec![message], &self.model)
            .with_system_prompt(TITLE_SYSTEM_PROMPT.to_string())
            .with_max_tokens(50)
            .with_temperature(0.3); // Lower temperature for more consistent titles

        let response = self.backend.send_message(&request).await?;

        // Extract the title from the response
        let title = response
            .message
            .text()
            .map(clean_title)
            .ok_or_else(|| BackendError::InvalidResponse("No title in response".to_string()))?;

        Ok(title)
    }
}

/// Truncate a string to a maximum length, adding ellipsis if truncated
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Clean up a generated title
fn clean_title(title: &str) -> String {
    title
        .trim()
        // Remove surrounding quotes
        .trim_matches('"')
        .trim_matches('\'')
        // Remove common prefixes the model might add
        .trim_start_matches("Title:")
        .trim_start_matches("title:")
        .trim()
        // Capitalize first letter
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
    }

    #[test]
    fn test_clean_title() {
        assert_eq!(clean_title("  My Title  "), "My Title");
        assert_eq!(clean_title("\"Quoted Title\""), "Quoted Title");
        assert_eq!(clean_title("Title: Some Title"), "Some Title");
    }
}
