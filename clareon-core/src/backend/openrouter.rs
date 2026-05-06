// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use openrouter_rs::OpenRouterClient;
use openrouter_rs::api::chat::{ChatCompletionRequest, Message as OrMessage};
use openrouter_rs::types::completion::Choice;
use openrouter_rs::types::{FinishReason, Role as OrRole, Tool, ToolCall as OrToolCall};
use tracing::{info, warn};

use super::traits::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, ModelInfo, ModelModalities, ModelPricing,
    StopReason, StreamEvent, ToolDefinition, Usage,
};
use crate::config::OpenAiBackendConfig;
use crate::error::BackendError;
use crate::types::{ContentBlock, ConversationId, Message, Role};

fn parse_owner(id: &str) -> Option<String> {
    id.split('/')
        .next()
        .filter(|_| id.contains('/'))
        .map(String::from)
}

/// Construct a `ToolCall` from its components via JSON deserialization.
///
/// `openrouter_rs::types::ToolCall` is `#[non_exhaustive]` and has no public
/// constructor in 0.9.0, so we go through `serde_json`.
fn make_tool_call(id: &str, name: &str, arguments: &str) -> OrToolCall {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments
        }
    }))
    .expect("ToolCall JSON structure is always valid")
}

/// OpenRouter-native API backend using the openrouter-rs crate
pub struct OpenRouterBackend {
    client: OpenRouterClient,
    default_model: ModelInfo,
}

impl OpenRouterBackend {
    pub fn from_config(config: &OpenAiBackendConfig) -> Result<Self, BackendError> {
        let api_key = config.api_key.clone().unwrap_or_default();

        let mut builder = OpenRouterClient::builder();
        builder.api_key(api_key);
        builder.http_referer("https://clareon.cc");
        builder.x_title("Clareon");
        builder.app_categories(["general-chat"]);
        if let Some(url) = &config.base_url {
            builder.base_url(url.as_str());
        }

        let client = builder.build().map_err(|e| BackendError::Api {
            status: 0,
            message: format!("Failed to build OpenRouter client: {e}"),
        })?;

        info!("Initializing OpenRouter backend");

        Ok(Self {
            client,
            default_model: Self::fallback_model(),
        })
    }

    fn fallback_model() -> ModelInfo {
        ModelInfo {
            id: "anthropic/claude-sonnet-4-5".to_string(),
            name: "Claude Sonnet 4.5".to_string(),
            context_window: 200_000,
            max_output_tokens: 8192,
            description: None,
            owner: Some("anthropic".to_string()),
            pricing: None,
            modalities: None,
        }
    }

    fn convert_messages(system_prompt: &Option<String>, messages: &[Message]) -> Vec<OrMessage> {
        let mut result = Vec::new();

        if let Some(prompt) = system_prompt {
            result.push(OrMessage::new(OrRole::System, prompt.as_str()));
        }

        for msg in messages {
            match msg.role {
                Role::User => {
                    let has_tool_results = msg.content.iter().any(|b| b.is_tool_result());

                    if has_tool_results {
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

                                    result.push(OrMessage::tool_response(
                                        tool_use_id.as_str(),
                                        tool_content.as_str(),
                                    ));
                                }
                                ContentBlock::Text { text } => {
                                    result.push(OrMessage::new(OrRole::User, text.as_str()));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        let text = msg
                            .content
                            .iter()
                            .filter_map(|b| b.as_text())
                            .collect::<Vec<_>>()
                            .join("\n");

                        if !text.is_empty() {
                            result.push(OrMessage::new(OrRole::User, text.as_str()));
                        }
                    }
                }
                Role::Assistant => {
                    let mut text_parts: Vec<String> = Vec::new();
                    let mut tool_calls: Vec<OrToolCall> = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => text_parts.push(text.clone()),
                            ContentBlock::ToolUse { id, name, input } => {
                                let arguments = serde_json::to_string(input)
                                    .unwrap_or_else(|_| "{}".to_string());
                                tool_calls.push(make_tool_call(id, name, &arguments));
                            }
                            _ => {}
                        }
                    }

                    let text = text_parts.join("\n");

                    if tool_calls.is_empty() {
                        result.push(OrMessage::new(OrRole::Assistant, text.as_str()));
                    } else {
                        result.push(OrMessage::assistant_with_tool_calls(
                            text.as_str(),
                            tool_calls,
                        ));
                    }
                }
            }
        }

        result
    }

    fn convert_tools(tools: &[ToolDefinition]) -> Option<Vec<Tool>> {
        if tools.is_empty() {
            return None;
        }
        Some(
            tools
                .iter()
                .map(|t| Tool::new(&t.name, &t.description, t.input_schema.clone()))
                .collect(),
        )
    }
}

fn map_openrouter_error(err: openrouter_rs::error::OpenRouterError) -> BackendError {
    use openrouter_rs::error::OpenRouterError;
    match err {
        OpenRouterError::Api(ctx) => {
            let status = ctx.status.as_u16();
            match status {
                401 => BackendError::Authentication(ctx.message.clone()),
                429 => BackendError::RateLimited {
                    retry_after_secs: None,
                },
                503 => BackendError::ServiceUnavailable,
                _ => BackendError::Api {
                    status,
                    message: ctx.message.clone(),
                },
            }
        }
        OpenRouterError::HttpRequest(_) => BackendError::ServiceUnavailable,
        other => BackendError::Api {
            status: 0,
            message: other.to_string(),
        },
    }
}

fn map_finish_reason(reason: &FinishReason) -> StopReason {
    match reason {
        FinishReason::Stop => StopReason::EndTurn,
        FinishReason::ToolCalls => StopReason::ToolUse,
        FinishReason::Length => StopReason::MaxTokens,
        FinishReason::ContentFilter | FinishReason::Error => StopReason::EndTurn,
    }
}

#[async_trait]
impl LlmBackend for OpenRouterBackend {
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError> {
        info!(
            "Sending message to OpenRouter API, model: {}",
            request.model
        );

        let messages = Self::convert_messages(&request.system_prompt, &request.messages);
        let tools = Self::convert_tools(&request.tools);

        let mut req_builder = ChatCompletionRequest::builder();
        req_builder.model(&request.model);
        req_builder.messages(messages);
        req_builder.max_tokens(request.max_tokens);

        if let Some(temp) = request.temperature {
            req_builder.temperature(f64::from(temp));
        }

        if let Some(tools) = tools {
            req_builder.tools(tools);
        }

        let or_request = req_builder.build().map_err(|e| BackendError::Api {
            status: 0,
            message: e.to_string(),
        })?;

        let response = self
            .client
            .send_chat_completion(&or_request)
            .await
            .map_err(map_openrouter_error)?;

        let conversation_id = request
            .messages
            .first()
            .map(|m| m.conversation_id.clone())
            .unwrap_or_else(|| ConversationId::from("temp"));

        let usage = response
            .usage
            .as_ref()
            .map(|u| Usage {
                input_tokens: i64::from(u.prompt_tokens),
                output_tokens: i64::from(u.completion_tokens),
                cache_read_input_tokens: None,
                cache_write_input_tokens: None,
            })
            .unwrap_or_default();

        let choice = response
            .choices
            .first()
            .ok_or_else(|| BackendError::InvalidResponse("No choices in response".to_string()))?;

        let Choice::NonStreaming(c) = choice else {
            return Err(BackendError::InvalidResponse(
                "Unexpected choice type in non-streaming response".to_string(),
            ));
        };

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        if let Some(text) = &c.message.content
            && !text.is_empty()
        {
            content_blocks.push(ContentBlock::Text { text: text.clone() });
        }

        if let Some(tool_calls) = &c.message.tool_calls {
            for tc in tool_calls {
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
        }

        if content_blocks.is_empty() {
            content_blocks.push(ContentBlock::Text {
                text: String::new(),
            });
        }

        let stop_reason = c
            .finish_reason
            .as_ref()
            .map(map_finish_reason)
            .unwrap_or(StopReason::EndTurn);

        let message = Message::assistant(
            conversation_id,
            content_blocks,
            &response.model,
            usage.input_tokens,
            usage.output_tokens,
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
            "Sending streaming message to OpenRouter API, model: {}",
            request.model
        );

        let messages = Self::convert_messages(&request.system_prompt, &request.messages);
        let tools = Self::convert_tools(&request.tools);

        let mut req_builder = ChatCompletionRequest::builder();
        req_builder.model(&request.model);
        req_builder.messages(messages);
        req_builder.max_tokens(request.max_tokens);

        if let Some(temp) = request.temperature {
            req_builder.temperature(f64::from(temp));
        }

        if let Some(tools) = tools {
            req_builder.tools(tools);
        }

        let or_request = req_builder.build().map_err(|e| BackendError::Api {
            status: 0,
            message: e.to_string(),
        })?;

        let stream = self
            .client
            .stream_chat_completion(&or_request)
            .await
            .map_err(map_openrouter_error)?;

        let started_blocks: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));

        let mapped_stream = stream.flat_map(move |result| {
            let started_blocks = Arc::clone(&started_blocks);
            let events: Vec<Result<StreamEvent, BackendError>> = match result {
                Err(err) => vec![Err(map_openrouter_error(err))],
                Ok(response) => {
                    let mut events = Vec::new();

                    if let Some(usage) = &response.usage {
                        events.push(Ok(StreamEvent::Usage(Usage {
                            input_tokens: i64::from(usage.prompt_tokens),
                            output_tokens: i64::from(usage.completion_tokens),
                            cache_read_input_tokens: None,
                            cache_write_input_tokens: None,
                        })));
                    }

                    for choice in &response.choices {
                        let Choice::Streaming(sc) = choice else {
                            continue;
                        };
                        let delta = &sc.delta;

                        if let Some(content) = &delta.content {
                            let mut started = started_blocks.lock().unwrap();
                            if !started.contains(&0) {
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

                        if let Some(tool_calls) = &delta.tool_calls {
                            for tc_chunk in tool_calls {
                                let block_index = tc_chunk.index.unwrap_or(0) as usize + 1;
                                let mut started = started_blocks.lock().unwrap();

                                if !started.contains(&block_index) {
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

                        if let Some(finish_reason) = &sc.finish_reason {
                            let started = started_blocks.lock().unwrap();
                            let mut indices: Vec<usize> = started.iter().copied().collect();
                            indices.sort_unstable();
                            for idx in indices {
                                events.push(Ok(StreamEvent::ContentBlockStop { index: idx }));
                            }
                            events.push(Ok(StreamEvent::MessageStop {
                                stop_reason: map_finish_reason(finish_reason),
                            }));
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
        "OpenRouter"
    }

    async fn available_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        let models = self
            .client
            .list_models()
            .await
            .map_err(map_openrouter_error)?;

        Ok(models
            .into_iter()
            .map(|m| {
                let owner = parse_owner(&m.id);
                ModelInfo {
                    name: m.name.clone(),
                    context_window: m.context_length as u32,
                    max_output_tokens: m.top_provider.max_completion_tokens.unwrap_or(0.0) as u32,
                    description: if m.description.is_empty() {
                        None
                    } else {
                        Some(m.description)
                    },
                    owner,
                    pricing: Some(ModelPricing {
                        prompt: Some(m.pricing.prompt),
                        completion: Some(m.pricing.completion),
                    }),
                    modalities: Some(ModelModalities {
                        input: vec![m.architecture.modality.clone()],
                        output: vec![m.architecture.modality],
                    }),
                    id: m.id,
                }
            })
            .collect())
    }

    fn default_model(&self) -> &ModelInfo {
        &self.default_model
    }
}
