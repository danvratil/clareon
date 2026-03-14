// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! OpenAI-compatible API backend implementation

use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_openai::Client;
use async_openai::config::OpenAIConfig as AsyncOpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestToolMessageContent,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    ChatCompletionResponseMessage, ChatCompletionStreamOptions, ChatCompletionTool,
    ChatCompletionTools, CreateChatCompletionRequestArgs, FinishReason, FunctionCall,
    FunctionObject,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use tracing::{debug, info, warn};

use super::traits::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, ModelInfo, StopReason, StreamEvent, Usage,
};
use crate::config::{OpenAiBackendConfig, Provider};
use crate::error::BackendError;
use crate::types::{ContentBlock, ConversationId, Message, Role};

/// OpenAI-compatible API backend
///
/// Used by multiple providers (OpenAI, OpenRouter, LiteLLM) with
/// provider-specific default configuration.
pub struct OpenAiBackend {
    client: Client<AsyncOpenAIConfig>,
    default_model: ModelInfo,
    #[allow(dead_code)] // Will be used for provider-aware model fetching
    provider: Provider,
}

impl OpenAiBackend {
    /// Create a new OpenAI backend from provider configuration
    ///
    /// The provider determines default settings (e.g., OpenRouter's base URL)
    /// which can be overridden by explicit config values.
    pub fn from_config(config: &OpenAiBackendConfig, provider: Provider) -> Self {
        let api_key = config.api_key.clone().unwrap_or_default();

        // Apply provider-specific default base URL if user hasn't set one
        let base_url = config.base_url.clone().or_else(|| match provider {
            Provider::OpenRouter => Some("https://openrouter.ai/api/v1".to_string()),
            _ => None,
        });

        let mut openai_config = AsyncOpenAIConfig::new().with_api_key(api_key);

        if let Some(url) = &base_url {
            openai_config = openai_config.with_api_base(url);
        }

        let client = Client::with_config(openai_config);

        info!("Initializing OpenAI backend (provider: {:?})", provider);

        Self {
            client,
            default_model: Self::fallback_model(),
            provider,
        }
    }

    /// Returns a static fallback model info
    fn fallback_model() -> ModelInfo {
        ModelInfo {
            id: "gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
            context_window: 128000,
            max_output_tokens: 16384,
            description: None,
            owner: None,
            pricing: None,
            modalities: None,
        }
    }

    /// Convert our Message types to OpenAI API format
    fn convert_messages(
        system_prompt: &Option<String>,
        messages: &[Message],
    ) -> Vec<ChatCompletionRequestMessage> {
        let mut result: Vec<ChatCompletionRequestMessage> = Vec::new();

        // Add system prompt as a system message if provided
        if let Some(prompt) = system_prompt {
            result.push(ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: prompt.as_str().into(),
                    name: None,
                },
            ));
        }

        for msg in messages {
            match msg.role {
                Role::User => {
                    // Check if the message contains tool results
                    let has_tool_results = msg.content.iter().any(|b| b.is_tool_result());

                    if has_tool_results {
                        // Tool results become separate Tool messages in OpenAI format
                        for block in &msg.content {
                            match block {
                                ContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                    is_error,
                                } => {
                                    let text = content
                                        .iter()
                                        .map(|c| match c {
                                            crate::types::ToolResultContent::Text { text } => {
                                                text.as_str()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");

                                    let tool_content = if is_error.unwrap_or(false) {
                                        format!("Error: {text}")
                                    } else {
                                        text
                                    };

                                    result.push(ChatCompletionRequestMessage::Tool(
                                        ChatCompletionRequestToolMessage {
                                            content: ChatCompletionRequestToolMessageContent::Text(
                                                tool_content,
                                            ),
                                            tool_call_id: tool_use_id.clone(),
                                        },
                                    ));
                                }
                                ContentBlock::Text { text } => {
                                    // Include any text blocks as user messages
                                    result.push(ChatCompletionRequestMessage::User(
                                        ChatCompletionRequestUserMessage {
                                            content: ChatCompletionRequestUserMessageContent::Text(
                                                text.clone(),
                                            ),
                                            name: None,
                                        },
                                    ));
                                }
                                _ => {
                                    // Skip other block types (Image, ToolUse) in user messages
                                }
                            }
                        }
                    } else {
                        // Regular user message - collect text content
                        let text = msg
                            .content
                            .iter()
                            .filter_map(|b| b.as_text())
                            .collect::<Vec<_>>()
                            .join("\n");

                        if !text.is_empty() {
                            result.push(ChatCompletionRequestMessage::User(
                                ChatCompletionRequestUserMessage {
                                    content: ChatCompletionRequestUserMessageContent::Text(text),
                                    name: None,
                                },
                            ));
                        }
                    }
                }
                Role::Assistant => {
                    // OpenAI puts tool_calls as a field on the assistant message
                    // Collect all text and tool_use blocks
                    let mut text_parts: Vec<String> = Vec::new();
                    let mut tool_calls: Vec<ChatCompletionMessageToolCalls> = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                let arguments = serde_json::to_string(input)
                                    .unwrap_or_else(|_| "{}".to_string());
                                tool_calls.push(ChatCompletionMessageToolCalls::Function(
                                    ChatCompletionMessageToolCall {
                                        id: id.clone(),
                                        function: FunctionCall {
                                            name: name.clone(),
                                            arguments,
                                        },
                                    },
                                ));
                            }
                            _ => {
                                // Skip other block types in assistant messages
                            }
                        }
                    }

                    let content = if text_parts.is_empty() {
                        None
                    } else {
                        Some(ChatCompletionRequestAssistantMessageContent::Text(
                            text_parts.join("\n"),
                        ))
                    };

                    let tool_calls_opt = if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    };

                    #[allow(deprecated)]
                    let assistant_msg = ChatCompletionRequestAssistantMessage {
                        content,
                        refusal: None,
                        name: None,
                        audio: None,
                        tool_calls: tool_calls_opt,
                        function_call: None,
                    };

                    result.push(ChatCompletionRequestMessage::Assistant(assistant_msg));
                }
            }
        }

        result
    }

    /// Convert tool definitions to OpenAI format
    fn convert_tools(tools: &[super::traits::ToolDefinition]) -> Option<Vec<ChatCompletionTools>> {
        if tools.is_empty() {
            return None;
        }

        Some(
            tools
                .iter()
                .map(|t| {
                    ChatCompletionTools::Function(ChatCompletionTool {
                        function: FunctionObject {
                            name: t.name.clone(),
                            description: Some(t.description.clone()),
                            parameters: Some(t.input_schema.clone()),
                            strict: None,
                        },
                    })
                })
                .collect(),
        )
    }

    /// Convert an OpenAI response message to our types
    fn convert_response(
        response_message: &ChatCompletionResponseMessage,
        model: &str,
        finish_reason: Option<FinishReason>,
        usage: Usage,
        conversation_id: ConversationId,
    ) -> (Message, StopReason) {
        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        // Add text content if present
        if let Some(text) = &response_message.content
            && !text.is_empty()
        {
            content_blocks.push(ContentBlock::Text { text: text.clone() });
        }

        // Add tool calls if present
        if let Some(tool_calls) = &response_message.tool_calls {
            for tool_call in tool_calls {
                match tool_call {
                    ChatCompletionMessageToolCalls::Function(tc) => {
                        let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or_else(|e| {
                                warn!("Failed to parse tool call arguments as JSON: {}", e);
                                serde_json::Value::Object(serde_json::Map::new())
                            });

                        content_blocks.push(ContentBlock::ToolUse {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            input,
                        });
                    }
                    ChatCompletionMessageToolCalls::Custom(ct) => {
                        // Map custom tool calls as well
                        let input: serde_json::Value = serde_json::from_str(&ct.custom_tool.input)
                            .unwrap_or_else(|e| {
                                warn!("Failed to parse custom tool call input as JSON: {}", e);
                                serde_json::Value::Object(serde_json::Map::new())
                            });

                        content_blocks.push(ContentBlock::ToolUse {
                            id: ct.id.clone(),
                            name: ct.custom_tool.name.clone(),
                            input,
                        });
                    }
                }
            }
        }

        // If no content blocks were generated, add an empty text block
        if content_blocks.is_empty() {
            content_blocks.push(ContentBlock::Text {
                text: String::new(),
            });
        }

        let stop_reason = match finish_reason {
            Some(FinishReason::Stop) => StopReason::EndTurn,
            Some(FinishReason::ToolCalls) => StopReason::ToolUse,
            Some(FinishReason::Length) => StopReason::MaxTokens,
            Some(FinishReason::ContentFilter) => StopReason::EndTurn,
            Some(FinishReason::FunctionCall) => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };

        let message = Message::assistant(
            conversation_id,
            content_blocks,
            model,
            usage.input_tokens,
            usage.output_tokens,
        );

        (message, stop_reason)
    }
}

/// Map an async_openai error to our BackendError
fn map_openai_error(err: async_openai::error::OpenAIError) -> BackendError {
    match err {
        async_openai::error::OpenAIError::ApiError(api_err) => {
            let status_code = api_err.code.as_deref().unwrap_or("");
            match status_code {
                "401" | "invalid_api_key" => BackendError::Authentication(api_err.message.clone()),
                "429" | "rate_limit_exceeded" => BackendError::RateLimited {
                    retry_after_secs: None,
                },
                "503" | "service_unavailable" => BackendError::ServiceUnavailable,
                "context_length_exceeded" => BackendError::ContextLengthExceeded { max_tokens: 0 },
                _ => BackendError::Api {
                    status: 0,
                    message: api_err.message.clone(),
                },
            }
        }
        async_openai::error::OpenAIError::Reqwest(e) => BackendError::Http(e),
        other => {
            // The async-openai crate may fail to deserialize error responses from
            // OpenAI-compatible APIs (e.g. OpenRouter returns `"code": 402` as integer
            // but the crate expects a string). Try to extract the error message from the
            // raw response body embedded in the error string.
            let error_str = other.to_string();
            if let Some(message) = try_extract_error_message(&error_str) {
                BackendError::Api { status: 0, message }
            } else {
                BackendError::Api {
                    status: 0,
                    message: error_str,
                }
            }
        }
    }
}

/// Try to extract a user-friendly error message from a raw error string that may
/// contain embedded JSON. OpenAI-compatible APIs often return error JSON that
/// async-openai fails to deserialize due to type mismatches.
fn try_extract_error_message(error_str: &str) -> Option<String> {
    // Look for JSON content embedded in the error string
    let json_start = error_str.find('{');
    let json_end = error_str.rfind('}');

    let start = json_start?;
    let end = json_end?;
    let json_str = &error_str[start..=end];
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    value
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

#[async_trait]
impl LlmBackend for OpenAiBackend {
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError> {
        info!("Sending message to OpenAI API, model: {}", request.model);
        debug!("Message count: {}", request.messages.len());
        debug!("Tools count: {}", request.tools.len());

        let messages = Self::convert_messages(&request.system_prompt, &request.messages);
        let tools = Self::convert_tools(&request.tools);

        #[allow(deprecated)]
        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(&request.model)
            .messages(messages)
            .max_completion_tokens(request.max_tokens);

        if let Some(temp) = request.temperature {
            builder.temperature(temp);
        }

        if let Some(tools) = tools {
            builder.tools(tools);
        }

        let openai_request = builder.build().map_err(map_openai_error)?;

        let response = self
            .client
            .chat()
            .create(openai_request)
            .await
            .map_err(map_openai_error)?;

        // Get conversation_id from the first message
        let conversation_id = request
            .messages
            .first()
            .map(|m| m.conversation_id.clone())
            .unwrap_or_else(|| ConversationId::from("temp"));

        let choice = response
            .choices
            .first()
            .ok_or_else(|| BackendError::InvalidResponse("No choices in response".to_string()))?;

        let usage = response
            .usage
            .map(|u| Usage {
                input_tokens: i64::from(u.prompt_tokens),
                output_tokens: i64::from(u.completion_tokens),
                cache_read_input_tokens: None,
                cache_write_input_tokens: None,
            })
            .unwrap_or_default();

        let (message, stop_reason) = Self::convert_response(
            &choice.message,
            &response.model,
            choice.finish_reason,
            usage,
            conversation_id,
        );

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
            "Sending streaming message to OpenAI API, model: {}",
            request.model
        );
        debug!("Message count: {}", request.messages.len());
        debug!("Tools count: {}", request.tools.len());

        let messages = Self::convert_messages(&request.system_prompt, &request.messages);
        let tools = Self::convert_tools(&request.tools);

        #[allow(deprecated)]
        let mut builder = CreateChatCompletionRequestArgs::default();
        builder
            .model(&request.model)
            .messages(messages)
            .max_completion_tokens(request.max_tokens)
            .stream_options(ChatCompletionStreamOptions {
                include_usage: Some(true),
                include_obfuscation: None,
            });

        if let Some(temp) = request.temperature {
            builder.temperature(temp);
        }

        if let Some(tools) = tools {
            builder.tools(tools);
        }

        let openai_request = builder.build().map_err(map_openai_error)?;

        let stream = self
            .client
            .chat()
            .create_stream(openai_request)
            .await
            .map_err(map_openai_error)?;

        // Track which content block indices have been started
        let started_blocks: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));

        let mapped_stream = stream.flat_map(move |result| {
            let started_blocks = Arc::clone(&started_blocks);
            let events: Vec<Result<StreamEvent, BackendError>> = match result {
                Err(err) => vec![Err(map_openai_error(err))],
                Ok(response) => {
                    let mut events = Vec::new();

                    // Handle usage in the final chunk
                    if let Some(usage) = response.usage {
                        events.push(Ok(StreamEvent::Usage(Usage {
                            input_tokens: i64::from(usage.prompt_tokens),
                            output_tokens: i64::from(usage.completion_tokens),
                            cache_read_input_tokens: None,
                            cache_write_input_tokens: None,
                        })));
                    }

                    for choice in &response.choices {
                        let delta = &choice.delta;

                        // Handle text content delta
                        if let Some(content) = &delta.content {
                            let mut started = started_blocks.lock().unwrap();
                            if !started.contains(&0) {
                                // First text delta - emit ContentBlockStart
                                started.insert(0);
                                events.push(Ok(StreamEvent::ContentBlockStart {
                                    index: 0,
                                    block: ContentBlock::Text {
                                        text: String::new(),
                                    },
                                }));
                            }
                            events.push(Ok(StreamEvent::ContentBlockDelta {
                                index: 0,
                                delta: ContentDelta::Text {
                                    text: content.clone(),
                                },
                            }));
                        }

                        // Handle tool call deltas
                        if let Some(tool_calls) = &delta.tool_calls {
                            for tc_chunk in tool_calls {
                                let block_index = tc_chunk.index as usize + 1;
                                let mut started = started_blocks.lock().unwrap();

                                if !started.contains(&block_index) {
                                    // First delta for this tool call - emit ContentBlockStart
                                    started.insert(block_index);
                                    let id = tc_chunk
                                        .id
                                        .clone()
                                        .unwrap_or_else(|| format!("tool_{block_index}"));
                                    let name = tc_chunk
                                        .function
                                        .as_ref()
                                        .and_then(|f| f.name.clone())
                                        .unwrap_or_default();
                                    events.push(Ok(StreamEvent::ContentBlockStart {
                                        index: block_index,
                                        block: ContentBlock::ToolUse {
                                            id,
                                            name,
                                            input: serde_json::Value::Object(
                                                serde_json::Map::new(),
                                            ),
                                        },
                                    }));
                                }

                                // Emit tool input delta if there are arguments
                                if let Some(func) = &tc_chunk.function
                                    && let Some(args) = &func.arguments
                                    && !args.is_empty()
                                {
                                    events.push(Ok(StreamEvent::ContentBlockDelta {
                                        index: block_index,
                                        delta: ContentDelta::ToolInput {
                                            partial_json: args.clone(),
                                        },
                                    }));
                                }
                            }
                        }

                        // Handle finish_reason
                        if let Some(finish_reason) = &choice.finish_reason {
                            let started = started_blocks.lock().unwrap();
                            let mut indices: Vec<usize> = started.iter().copied().collect();
                            indices.sort_unstable();
                            for idx in indices {
                                events.push(Ok(StreamEvent::ContentBlockStop { index: idx }));
                            }

                            let stop_reason = match finish_reason {
                                FinishReason::Stop => StopReason::EndTurn,
                                FinishReason::ToolCalls => StopReason::ToolUse,
                                FinishReason::Length => StopReason::MaxTokens,
                                FinishReason::ContentFilter => StopReason::EndTurn,
                                FinishReason::FunctionCall => StopReason::ToolUse,
                            };
                            events.push(Ok(StreamEvent::MessageStop { stop_reason }));
                        }
                    }

                    events
                }
            };

            futures::stream::iter(events)
        });

        Ok(Box::pin(mapped_stream))
    }

    fn name(&self) -> &'static str {
        "OpenAI"
    }

    async fn available_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        let response = self
            .client
            .models()
            .list()
            .await
            .map_err(map_openai_error)?;

        let models = response
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.id,
                context_window: 0, // OpenAI API doesn't provide this in the list endpoint
                max_output_tokens: 0,
                description: None,
                owner: None,
                pricing: None,
                modalities: None,
            })
            .collect();

        Ok(models)
    }

    fn default_model(&self) -> &ModelInfo {
        &self.default_model
    }
}
