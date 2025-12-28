// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Anthropic API backend implementation

use std::pin::Pin;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::traits::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, ModelInfo, StopReason, StreamEvent, Usage,
};
use crate::error::BackendError;
use crate::types::{ContentBlock, ConversationId, Message, Role};

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
        conversation_id: ConversationId,
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
            cache_read_input_tokens: response.usage.cache_read_input_tokens,
            cache_write_input_tokens: response.usage.cache_creation_input_tokens,
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

    /// Convert Anthropic stream event to our StreamEvent type
    /// Returns Ok(Some(event)) for events to emit, Ok(None) for events to skip
    fn convert_stream_event(
        event: AnthropicStreamEvent,
    ) -> Result<Option<StreamEvent>, BackendError> {
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                // Emit initial usage
                Ok(Some(StreamEvent::Usage(Usage {
                    input_tokens: message.usage.input_tokens,
                    output_tokens: message.usage.output_tokens,
                    cache_read_input_tokens: message.usage.cache_read_input_tokens,
                    cache_write_input_tokens: message.usage.cache_creation_input_tokens,
                })))
            }
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let block = match content_block {
                    ContentBlockStart::Text { text } => ContentBlock::Text { text },
                    ContentBlockStart::ToolUse { id, name, input } => {
                        ContentBlock::ToolUse { id, name, input }
                    }
                };
                Ok(Some(StreamEvent::ContentBlockStart { index, block }))
            }
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                let content_delta = match delta {
                    Delta::TextDelta { text } => ContentDelta::Text { text },
                    Delta::InputJsonDelta { partial_json } => {
                        ContentDelta::ToolInput { partial_json }
                    }
                };
                Ok(Some(StreamEvent::ContentBlockDelta {
                    index,
                    delta: content_delta,
                }))
            }
            AnthropicStreamEvent::ContentBlockStop { index } => {
                Ok(Some(StreamEvent::ContentBlockStop { index }))
            }
            AnthropicStreamEvent::MessageDelta { delta, .. } => {
                // If there's a stop reason, emit MessageStop
                if let Some(reason_str) = delta.stop_reason {
                    let stop_reason = match reason_str.as_str() {
                        "end_turn" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        "stop_sequence" => StopReason::StopSequence,
                        _ => StopReason::EndTurn,
                    };
                    Ok(Some(StreamEvent::MessageStop { stop_reason }))
                } else {
                    // No stop reason yet, skip this event (just a usage update)
                    Ok(None)
                }
            }
            AnthropicStreamEvent::MessageStop {} | AnthropicStreamEvent::Ping {} => {
                // These don't map to our events - skip them
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl LlmBackend for AnthropicBackend {
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError> {
        info!("Sending message to Anthropic API, model: {}", request.model);
        debug!("Message count: {}", request.messages.len());
        debug!("Tools count: {}", request.tools.len());

        let api_request = AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            system: request.system_prompt.clone(),
            messages: Self::convert_messages(&request.messages),
            temperature: request.temperature,
            stream: Some(false),
            tools: request
                .tools
                .iter()
                .map(|t| AnthropicTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.input_schema.clone(),
                })
                .collect(),
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
        let conversation_id = request
            .messages
            .first()
            .map(|m| m.conversation_id.clone())
            .unwrap_or_else(|| ConversationId::from("temp"));

        let (message, stop_reason, usage) = Self::convert_response(api_response, conversation_id);

        Ok(ChatResponse {
            message,
            stop_reason,
            usage,
        })
    }

    async fn send_message_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, BackendError>> + Send>>, BackendError>
    {
        info!(
            "Streaming message to Anthropic API, model: {}",
            request.model
        );
        debug!("Message count: {}", request.messages.len());
        debug!("Tools count: {}", request.tools.len());

        let api_request = AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            system: request.system_prompt.clone(),
            messages: Self::convert_messages(&request.messages),
            temperature: request.temperature,
            stream: Some(true),
            tools: request
                .tools
                .iter()
                .map(|t| AnthropicTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.input_schema.clone(),
                })
                .collect(),
        };

        // Send the request
        let response = self
            .client
            .post(&self.base_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&api_request)
            .send()
            .await?;

        // Check status before starting to stream
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

        // Create SSE event stream
        let event_stream = response.bytes_stream().eventsource();

        // Convert SSE events to our StreamEvent type
        let stream = event_stream
            .map(|result| {
                match result {
                    Ok(event) => {
                        // Skip empty data
                        if event.data.is_empty() {
                            return Ok(None);
                        }

                        // Parse the JSON event
                        let anthropic_event: AnthropicStreamEvent =
                            serde_json::from_str(&event.data).map_err(|e| {
                                warn!("Failed to parse SSE event: {}", e);
                                BackendError::InvalidResponse(format!(
                                    "Invalid JSON in SSE event: {}",
                                    e
                                ))
                            })?;

                        // Convert to our StreamEvent (returns Option)
                        Self::convert_stream_event(anthropic_event)
                    }
                    Err(e) => {
                        warn!("SSE stream error: {}", e);
                        Err(BackendError::InvalidResponse(format!(
                            "SSE stream error: {}",
                            e
                        )))
                    }
                }
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(event)) => Some(Ok(event)),
                    Ok(None) => None, // Skip None events
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(Box::pin(stream))
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
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
    /// Tokens read from cache (prompt caching)
    #[serde(default)]
    cache_read_input_tokens: Option<i64>,
    /// Tokens written to cache (prompt caching)
    #[serde(default)]
    cache_creation_input_tokens: Option<i64>,
}

// Streaming event types

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStart },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStart,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaContent,
        #[allow(dead_code)]
        usage: DeltaUsage,
    },
    #[serde(rename = "message_stop")]
    MessageStop {},
    #[serde(rename = "ping")]
    Ping {},
}

#[derive(Debug, Deserialize)]
struct MessageStart {
    #[allow(dead_code)]
    id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    type_: String,
    #[allow(dead_code)]
    role: String,
    #[allow(dead_code)]
    model: String,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlockStart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct MessageDeltaContent {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeltaUsage {
    #[allow(dead_code)]
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
