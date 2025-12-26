//! Anthropic API backend implementation

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::traits::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, ModelInfo, StopReason, StreamEvent, Usage,
};
use crate::error::BackendError;
use crate::types::{ContentBlock, Message, Role};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API backend
pub struct AnthropicBackend {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicBackend {
    /// Create a new Anthropic backend with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: ANTHROPIC_API_URL.to_string(),
        }
    }

    /// Create a new Anthropic backend with a custom base URL
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }

    /// Convert our Message type to Anthropic API format
    fn convert_messages(messages: &[Message]) -> Vec<AnthropicMessage> {
        messages
            .iter()
            .map(|msg| AnthropicMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content: msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => AnthropicContent::Text {
                            r#type: "text".to_string(),
                            text: text.clone(),
                        },
                        ContentBlock::ToolUse { id, name, input } => AnthropicContent::ToolUse {
                            r#type: "tool_use".to_string(),
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => AnthropicContent::ToolResult {
                            r#type: "tool_result".to_string(),
                            tool_use_id: tool_use_id.clone(),
                            content: content
                                .iter()
                                .map(|c| match c {
                                    crate::types::ToolResultContent::Text { text } => {
                                        AnthropicToolResultContent::Text {
                                            r#type: "text".to_string(),
                                            text: text.clone(),
                                        }
                                    }
                                })
                                .collect(),
                            is_error: *is_error,
                        },
                    })
                    .collect(),
            })
            .collect()
    }

    /// Convert Anthropic response to our types
    fn convert_response(
        response: AnthropicResponse,
        conversation_id: i64,
    ) -> (Message, StopReason, Usage) {
        let content: Vec<ContentBlock> = response
            .content
            .iter()
            .map(|block| match block {
                AnthropicContent::Text { text, .. } => ContentBlock::Text { text: text.clone() },
                AnthropicContent::ToolUse {
                    id, name, input, ..
                } => ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                },
                AnthropicContent::ToolResult { .. } => {
                    // Tool results shouldn't appear in responses
                    ContentBlock::Text {
                        text: "[invalid tool result in response]".to_string(),
                    }
                }
            })
            .collect();

        let stop_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            Some("stop_sequence") => StopReason::StopSequence,
            _ => StopReason::EndTurn,
        };

        let usage = Usage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
        };

        let message = Message::assistant(
            conversation_id,
            content,
            &response.model,
            usage.input_tokens,
            usage.output_tokens,
        );

        (message, stop_reason, usage)
    }
}

#[async_trait]
impl LlmBackend for AnthropicBackend {
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError> {
        info!("Sending message to Anthropic API, model: {}", request.model);
        debug!("Message count: {}", request.messages.len());

        let api_request = AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            system: request.system_prompt.clone(),
            messages: Self::convert_messages(&request.messages),
            temperature: request.temperature,
            stream: Some(false),
        };

        let response = self
            .client
            .post(&self.base_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&api_request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            return match status.as_u16() {
                401 => Err(BackendError::Authentication(error_text)),
                429 => Err(BackendError::RateLimited {
                    retry_after_secs: None,
                }),
                500..=599 => Err(BackendError::ServiceUnavailable),
                _ => Err(BackendError::Api {
                    status: status.as_u16(),
                    message: error_text,
                }),
            };
        }

        let api_response: AnthropicResponse = response.json().await?;

        // Get conversation_id from the first message (they should all have the same one)
        let conversation_id = request.messages.first().map(|m| m.conversation_id).unwrap_or(0);

        let (message, stop_reason, usage) = Self::convert_response(api_response, conversation_id);

        Ok(ChatResponse {
            message,
            stop_reason,
            usage,
        })
    }

    async fn send_message_stream(
        &self,
        _request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, BackendError>> + Send>>, BackendError>
    {
        // TODO: Implement streaming
        Err(BackendError::InvalidResponse(
            "Streaming not yet implemented for Anthropic backend".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "Anthropic API"
    }

    fn available_models(&self) -> &[ModelInfo] {
        &ANTHROPIC_MODELS
    }
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text {
        r#type: String,
        text: String,
    },
    ToolUse {
        r#type: String,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        r#type: String,
        tool_use_id: String,
        content: Vec<AnthropicToolResultContent>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicToolResultContent {
    Text { r#type: String, text: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    model: String,
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: i64,
    output_tokens: i64,
}

// Available models

static ANTHROPIC_MODELS: [ModelInfo; 4] = [
    ModelInfo {
        id: String::new(), // Will be replaced with proper const initialization
        name: String::new(),
        context_window: 200000,
        max_output_tokens: 8192,
    },
    ModelInfo {
        id: String::new(),
        name: String::new(),
        context_window: 200000,
        max_output_tokens: 8192,
    },
    ModelInfo {
        id: String::new(),
        name: String::new(),
        context_window: 200000,
        max_output_tokens: 4096,
    },
    ModelInfo {
        id: String::new(),
        name: String::new(),
        context_window: 200000,
        max_output_tokens: 4096,
    },
];

// Helper function to get model info dynamically
impl AnthropicBackend {
    /// Get available model information
    pub fn get_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                name: "Claude Opus 4".to_string(),
                context_window: 200000,
                max_output_tokens: 32000,
            },
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                context_window: 200000,
                max_output_tokens: 16000,
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                context_window: 200000,
                max_output_tokens: 8192,
            },
        ]
    }
}
