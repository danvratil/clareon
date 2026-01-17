// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backend trait definitions

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::error::BackendError;
use crate::types::{ContentBlock, Message};

/// Trait for LLM backend implementations
#[async_trait]
pub trait LlmBackend: Send + Sync {
    /// Send a message and get a complete response
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError>;

    /// Send a message with streaming response
    ///
    /// Returns a stream of events that can be processed as they arrive.
    /// This is useful for displaying responses in real-time.
    async fn send_message_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, BackendError>> + Send>>, BackendError>;

    /// Get the backend name (for display/logging)
    fn name(&self) -> &'static str;

    /// List available models for this backend
    fn available_models(&self) -> &[ModelInfo];

    /// Return the default model for this backend
    fn default_model(&self) -> &ModelInfo;
}

/// Request to send to the LLM
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Conversation messages
    pub messages: Vec<Message>,

    /// System prompt (optional, uses default if None)
    pub system_prompt: Option<String>,

    /// Model identifier
    pub model: String,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Tool definitions (empty for MVP)
    pub tools: Vec<ToolDefinition>,

    /// Temperature (0.0 - 1.0)
    pub temperature: Option<f32>,
}

impl ChatRequest {
    /// Create a new chat request with default settings
    pub fn new(messages: Vec<Message>, model: impl Into<String>) -> Self {
        Self {
            messages,
            system_prompt: None,
            model: model.into(),
            max_tokens: 4096,
            tools: Vec::new(),
            temperature: None,
        }
    }

    /// Set the system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

/// Response from the LLM
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// The generated message
    pub message: Message,

    /// Why the model stopped generating
    pub stop_reason: StopReason,

    /// Token usage information
    pub usage: Usage,
}

/// Reason why the model stopped generating
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of response
    EndTurn,
    /// Model wants to use a tool
    ToolUse,
    /// Hit the max_tokens limit
    MaxTokens,
    /// Hit a stop sequence
    StopSequence,
}

/// Token usage information
#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    /// Tokens in the input (prompt)
    pub input_tokens: i64,
    /// Tokens in the output (response)
    pub output_tokens: i64,
    /// Input tokens read from cache (for prompt caching)
    pub cache_read_input_tokens: Option<i64>,
    /// Input tokens written to cache (for prompt caching)
    pub cache_write_input_tokens: Option<i64>,
}

/// Information about an available model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model identifier to use in API calls
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Maximum context window size
    pub context_window: u32,
    /// Maximum output tokens
    pub max_output_tokens: u32,
}

/// Tool definition (for future tool support)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
}

/// Streaming event from the LLM
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Start of a content block
    ContentBlockStart { index: usize, block: ContentBlock },
    /// Delta update to a content block
    ContentBlockDelta { index: usize, delta: ContentDelta },
    /// End of a content block
    ContentBlockStop { index: usize },
    /// End of the message
    MessageStop { stop_reason: StopReason },
    /// Usage information (usually sent at the end)
    Usage(Usage),
}

/// Delta update for streaming
#[derive(Debug, Clone)]
pub enum ContentDelta {
    /// Text delta
    Text { text: String },
    /// Tool input delta (JSON fragment)
    ToolInput { partial_json: String },
}
