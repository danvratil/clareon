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
use crate::config::{ANTHROPIC_API_KEY, AnthropicConfig, SecretStore};
use crate::error::BackendError;
use crate::types::{ContentBlock, ConversationId, Message, Role};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic API backend
pub struct AnthropicBackend {
    client: Client,
    api_key: String,
    base_url: String,
    models: Vec<ModelInfo>,
}

impl AnthropicBackend {
    /// Create a new Anthropic backend from configuration
    ///
    /// This will retrieve the API key from either the system keyring (if configured)
    /// or fall back to the ANTHROPIC_API_KEY environment variable.
    pub async fn from_config(config: &AnthropicConfig) -> Result<Self, BackendError> {
        let api_key = if config.api_key_in_keyring {
            // Try to get from keyring first
            match SecretStore::new().await {
                Ok(store) => match store.get_secret(ANTHROPIC_API_KEY).await {
                    Ok(key) => {
                        debug!("Retrieved API key from keyring");
                        key
                    }
                    Err(_) => {
                        // Fall back to environment variable
                        debug!("API key not found in keyring, falling back to environment");
                        std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                            BackendError::Configuration(
                                "ANTHROPIC_API_KEY not found in keyring or environment".to_string(),
                            )
                        })?
                    }
                },
                Err(_) => {
                    // If secret service is not available, fall back to environment
                    debug!("Secret service not available, using environment variable");
                    std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                        BackendError::Configuration(
                            "ANTHROPIC_API_KEY environment variable not set".to_string(),
                        )
                    })?
                }
            }
        } else {
            // Use environment variable
            std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                BackendError::Configuration(
                    "ANTHROPIC_API_KEY environment variable not set".to_string(),
                )
            })?
        };

        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| ANTHROPIC_API_URL.to_string());

        info!("Initializing Anthropic backend with base URL: {}", base_url);

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url,
            models: get_anthropic_models(),
        })
    }

    /// Create a new Anthropic backend with the given API key
    ///
    /// This is a convenience constructor for testing and CLI. Use `from_config` in production.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: ANTHROPIC_API_URL.to_string(),
            models: get_anthropic_models(),
        }
    }

    /// Create a new Anthropic backend with a custom base URL
    ///
    /// This is a convenience constructor for testing and CLI. Use `from_config` in production.
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            models: get_anthropic_models(),
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
            AnthropicStreamEvent::Error { error } => match error.r#type.as_str() {
                "authentication_error" => Err(BackendError::Authentication(error.message)),
                "rate_limit_error" => Err(BackendError::RateLimited {
                    retry_after_secs: None,
                }),
                "service_unavailable_error" => Err(BackendError::ServiceUnavailable),
                _ => Err(BackendError::Api {
                    status: 0,
                    message: error.message,
                }),
            },
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
            .post(format!("{}/v1/messages", self.base_url))
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
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&api_request)
            .send()
            .await?;

        // Check status before starting to stream
        let status = response.status();
        if !status.is_success() {
            let headers = response.headers().clone();
            let error_text = response.text().await.unwrap_or_default();
            let error = match serde_json::from_str::<ErrorResponse>(&error_text) {
                Ok(error) => error.error,
                Err(_) => ErrorShape {
                    r#type: "unknown".to_string(),
                    message: format!("Failed to parse error response: {error_text}"),
                },
            };

            return match status.as_u16() {
                401 => Err(BackendError::Authentication(error.message)),
                429 => Err(BackendError::RateLimited {
                    retry_after_secs: headers
                        .get("retry-after")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse().ok()),
                }),
                500..=599 => Err(BackendError::ServiceUnavailable),
                _ => Err(BackendError::Api {
                    status: status.as_u16(),
                    message: error.message,
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
        &self.models
    }

    fn default_model(&self) -> &ModelInfo {
        // Return Sonnet as the default model
        self.models
            .iter()
            .find(|m| m.id.contains("sonnet"))
            .unwrap_or_else(|| &self.models[0])
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
    MessageStart {
        message: MessageStart,
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStart,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: usize,
        delta: Delta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        index: usize,
    },
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
    Error {
        error: ErrorShape,
    },
}

#[derive(Debug, Deserialize)]
struct ErrorShape {
    // TODO: turn this into an enum
    r#type: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorShape,
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
// Use a function instead of static array to avoid const initialization issues with String
fn get_anthropic_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "claude-opus-4-5-20251101".to_string(),
            name: "Claude Opus 4.5".to_string(),
            context_window: 200000,
            max_output_tokens: 32000,
        },
        ModelInfo {
            id: "claude-sonnet-4-5-20250929".to_string(),
            name: "Claude Sonnet 4.5".to_string(),
            context_window: 200000,
            max_output_tokens: 16000,
        },
        ModelInfo {
            id: "claude-haiku-4-5-20251001".to_string(),
            name: "Claude Haiku 4.5".to_string(),
            context_window: 200000,
            max_output_tokens: 8192,
        },
    ]
}
