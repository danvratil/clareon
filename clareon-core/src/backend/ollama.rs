// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Ollama backend implementation (native API via ollama-rs)
//!
//! Construction is synchronous and infallible — no network I/O at startup.
//! Errors are surfaced lazily from `available_models()` and `send_message*()`.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use ollama_rs::generation::images::Image;
use ollama_rs::generation::tools::{
    ToolCall, ToolCallFunction, ToolFunctionInfo, ToolInfo, ToolType,
};
use tokio::sync::OnceCell;
use tracing::{info, warn};
use url::Url;

use super::traits::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, ModelInfo, StopReason, StreamEvent,
    ToolDefinition, Usage,
};
use crate::config::OllamaConfig;
use crate::error::BackendError;
use crate::types::{ContentBlock, ConversationId, ImageSource, Message, Role, ToolResultContent};

const DEFAULT_HOST: &str = "http://localhost";
const DEFAULT_PORT: u16 = 11434;

/// Native Ollama backend (HTTP API on localhost or remote daemon).
pub struct OllamaBackend {
    client: Ollama,
    base_url_for_diagnostics: String,
    configured_default_model: Option<String>,
    placeholder_model: ModelInfo,
    cached_default_model: OnceCell<ModelInfo>,
}

impl OllamaBackend {
    /// Construct from configuration. Infallible and synchronous —
    /// performs no network I/O.
    pub fn from_config(config: &OllamaConfig) -> Self {
        let (host, port, base_url_for_diagnostics) = parse_base_url(config.base_url.as_deref());

        info!(
            "Initializing Ollama backend at {} (lazy — no daemon contact yet)",
            base_url_for_diagnostics
        );

        Self {
            client: Ollama::new(host, port),
            base_url_for_diagnostics,
            configured_default_model: config.default_model.clone(),
            placeholder_model: placeholder_model_info(),
            cached_default_model: OnceCell::new(),
        }
    }
}

/// Parse the optional `base_url` into (host_with_scheme, port, diagnostic_url).
///
/// `Ollama::new` accepts a host **with** scheme (e.g. `"http://localhost"`)
/// and a port as a separate argument.
fn parse_base_url(raw: Option<&str>) -> (String, u16, String) {
    let raw = raw.unwrap_or("");
    if raw.is_empty() {
        let full = format!("{DEFAULT_HOST}:{DEFAULT_PORT}");
        return (DEFAULT_HOST.to_string(), DEFAULT_PORT, full);
    }

    match Url::parse(raw) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("localhost");
            let port = parsed.port_or_known_default().unwrap_or(DEFAULT_PORT);
            let host_with_scheme = format!("{scheme}://{host}");
            let diag = format!("{host_with_scheme}:{port}");
            (host_with_scheme, port, diag)
        }
        Err(_) => {
            // Fall back to defaults if user provided a malformed URL.
            let full = format!("{DEFAULT_HOST}:{DEFAULT_PORT}");
            (DEFAULT_HOST.to_string(), DEFAULT_PORT, full)
        }
    }
}

/// Parse an Ollama model name to extract a namespace/owner prefix.
///
/// Examples:
/// - `"llama3.2"` → `None`
/// - `"library/llama3.2"` → `Some("library")`
/// - `"hf.co/user/model:tag"` → `Some("hf.co")`
fn parse_owner(name: &str) -> Option<String> {
    name.split_once('/').map(|(owner, _)| owner.to_string())
}

/// Convert a single `LocalModel` into our `ModelInfo`.
///
/// Ollama's list endpoint does not return context-window size, so we
/// leave `context_window` and `max_output_tokens` at 0. A future
/// enhancement can call `show_model_info` per model to fill these.
fn local_model_to_model_info(model: ollama_rs::models::LocalModel) -> ModelInfo {
    let owner = parse_owner(&model.name);
    // LocalModel in ollama-rs 0.3.4 has no `details` field with parameter_size /
    // quantization_level, so we leave description as None.
    let description: Option<String> = None;

    ModelInfo {
        id: model.name.clone(),
        name: model.name,
        context_window: 0,
        max_output_tokens: 0,
        description,
        owner,
        pricing: None,
        modalities: None,
    }
}

/// Map an `ollama_rs::error::OllamaError` to our `BackendError`.
///
/// Connection-refused or other transport failures are mapped to
/// `BackendError::ServiceUnavailable` with a `warn!` log including
/// the daemon URL hint. Other errors degrade to `Api` or `InvalidResponse`.
fn map_ollama_error(err: ollama_rs::error::OllamaError, base_url: &str) -> BackendError {
    use ollama_rs::error::OllamaError;
    match err {
        OllamaError::ReqwestError(e) if e.is_connect() || e.is_timeout() => {
            warn!(
                "Cannot reach Ollama at {}. Is it running? Start it with `ollama serve`. Underlying: {}",
                base_url, e
            );
            BackendError::ServiceUnavailable
        }
        // ollama-rs uses reqwest 0.12 while clareon-core uses reqwest 0.13;
        // the two Error types are incompatible, so we downgrade to InvalidResponse.
        OllamaError::ReqwestError(e) => BackendError::InvalidResponse(format!("HTTP error: {e}")),
        OllamaError::JsonError(e) => {
            BackendError::InvalidResponse(format!("JSON parse error: {e}"))
        }
        other => BackendError::InvalidResponse(other.to_string()),
    }
}

fn placeholder_model_info() -> ModelInfo {
    ModelInfo {
        id: String::new(),
        name: "(no model available)".to_string(),
        context_window: 0,
        max_output_tokens: 0,
        description: Some(
            "No Ollama model has been resolved yet. Pull a model with `ollama pull <name>`."
                .to_string(),
        ),
        owner: None,
        pricing: None,
        modalities: None,
    }
}

/// Choose a default model from a list of locally available models.
///
/// If `configured` is `Some(name)`, looks it up in `available` and returns
/// it; if not found, returns `ModelNotAvailable(name)`. If `configured` is
/// `None`, returns the first available model, or `ModelNotAvailable` with
/// a hint if the list is empty.
fn select_default_model(
    configured: Option<&str>,
    available: &[ModelInfo],
) -> Result<ModelInfo, BackendError> {
    match configured {
        Some(name) => available
            .iter()
            .find(|m| m.id == name)
            .cloned()
            .ok_or_else(|| BackendError::ModelNotAvailable(name.to_string())),
        None => available.first().cloned().ok_or_else(|| {
            BackendError::ModelNotAvailable(
                "No Ollama models installed. Pull one with `ollama pull <model>`.".to_string(),
            )
        }),
    }
}

impl OllamaBackend {
    /// Resolve the default model lazily.
    ///
    /// On first call, queries `available_models()` and picks either the
    /// configured default or the first available model. Subsequent calls
    /// return the cached `ModelInfo`.
    async fn resolve_default_model(&self) -> Result<&ModelInfo, BackendError> {
        self.cached_default_model
            .get_or_try_init(|| async {
                let models = self.available_models().await?;
                select_default_model(self.configured_default_model.as_deref(), &models)
            })
            .await
    }
}

#[async_trait]
impl LlmBackend for OllamaBackend {
    async fn send_message(&self, request: &ChatRequest) -> Result<ChatResponse, BackendError> {
        use ollama_rs::generation::chat::request::ChatMessageRequest;
        use ollama_rs::models::ModelOptions;

        info!(
            "Sending message to Ollama at {}, model: {}",
            self.base_url_for_diagnostics, request.model
        );

        let model = if request.model.is_empty() {
            self.resolve_default_model().await?.id.clone()
        } else {
            request.model.clone()
        };

        let messages = convert_messages(&request.system_prompt, &request.messages);

        let mut options = ModelOptions::default()
            .num_predict(i32::try_from(request.max_tokens).unwrap_or(i32::MAX));
        if let Some(temp) = request.temperature {
            options = options.temperature(temp);
        }

        let mut ollama_req = ChatMessageRequest::new(model.clone(), messages).options(options);

        let tools = convert_tools(&request.tools);
        if !tools.is_empty() {
            ollama_req = ollama_req.tools(tools);
        }

        let response = self
            .client
            .send_chat_messages(ollama_req)
            .await
            .map_err(|e| map_ollama_error(e, &self.base_url_for_diagnostics))?;

        let conversation_id = request
            .messages
            .first()
            .map(|m| m.conversation_id.clone())
            .unwrap_or_else(|| ConversationId::from("temp"));

        let (prompt_eval_count, eval_count) = response
            .final_data
            .as_ref()
            .map(|d| (Some(d.prompt_eval_count), Some(d.eval_count)))
            .unwrap_or((None, None));

        let (message, stop_reason, usage) = convert_response(
            response.message,
            &model,
            prompt_eval_count,
            eval_count,
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
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};

        use futures::StreamExt;
        use ollama_rs::generation::chat::request::ChatMessageRequest;
        use ollama_rs::models::ModelOptions;

        info!(
            "Sending streaming message to Ollama at {}, model: {}",
            self.base_url_for_diagnostics, request.model
        );

        let model = if request.model.is_empty() {
            self.resolve_default_model().await?.id.clone()
        } else {
            request.model.clone()
        };

        let messages = convert_messages(&request.system_prompt, &request.messages);

        let mut options = ModelOptions::default()
            .num_predict(i32::try_from(request.max_tokens).unwrap_or(i32::MAX));
        if let Some(temp) = request.temperature {
            options = options.temperature(temp);
        }

        let mut ollama_req = ChatMessageRequest::new(model.clone(), messages).options(options);

        let tools = convert_tools(&request.tools);
        if !tools.is_empty() {
            ollama_req = ollama_req.tools(tools);
        }

        let base_url = self.base_url_for_diagnostics.clone();
        let stream = self
            .client
            .send_chat_messages_stream(ollama_req)
            .await
            .map_err(|e| map_ollama_error(e, &base_url))?;

        // Track which content block indices have been started
        let started: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));
        let any_tool_calls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let mapped = stream.flat_map(move |result| {
            let started = Arc::clone(&started);
            let any_tool_calls = Arc::clone(&any_tool_calls);
            let base_url = base_url.clone();

            let events: Vec<Result<StreamEvent, BackendError>> = match result {
                // The ollama-rs stream yields Err(()) for transport errors; we
                // can only report them as a generic message since no details are
                // available (the crate prints them to stderr internally).
                Err(()) => vec![Err(BackendError::InvalidResponse(format!(
                    "Streaming error from Ollama at {base_url}"
                )))],
                Ok(chunk) => {
                    let mut events = Vec::new();

                    // Text delta — each chunk carries a small piece, not the
                    // accumulated text.
                    if !chunk.message.content.is_empty() {
                        let mut s = started.lock().unwrap();
                        if !s.contains(&0) {
                            s.insert(0);
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
                                text: chunk.message.content.clone(),
                            },
                        }));
                    }

                    // Tool calls — Ollama emits them complete in a single chunk;
                    // one tool-call per ContentBlock with index >= 1.
                    for (i, tc) in chunk.message.tool_calls.into_iter().enumerate() {
                        let block_index = i + 1;
                        {
                            let mut s = started.lock().unwrap();
                            if !s.contains(&block_index) {
                                s.insert(block_index);
                                *any_tool_calls.lock().unwrap() = true;
                                events.push(Ok(StreamEvent::ContentBlockStart {
                                    index: block_index,
                                    block: ContentBlock::ToolUse {
                                        id: format!("call_{i}"),
                                        name: tc.function.name.clone(),
                                        input: serde_json::Value::Object(serde_json::Map::new()),
                                    },
                                }));
                            }
                        }
                        events.push(Ok(StreamEvent::ContentBlockDelta {
                            index: block_index,
                            delta: ContentDelta::ToolInput {
                                partial_json: tc.function.arguments.to_string(),
                            },
                        }));
                    }

                    // Final chunk: emit stops + usage + message_stop
                    if chunk.done {
                        let s = started.lock().unwrap();
                        let mut indices: Vec<usize> = s.iter().copied().collect();
                        indices.sort_unstable();
                        for idx in indices {
                            events.push(Ok(StreamEvent::ContentBlockStop { index: idx }));
                        }

                        let (prompt_eval, eval) = chunk
                            .final_data
                            .as_ref()
                            .map(|d| (Some(d.prompt_eval_count), Some(d.eval_count)))
                            .unwrap_or((None, None));

                        events.push(Ok(StreamEvent::Usage(Usage {
                            input_tokens: prompt_eval.unwrap_or(0) as i64,
                            output_tokens: eval.unwrap_or(0) as i64,
                            cache_read_input_tokens: None,
                            cache_write_input_tokens: None,
                        })));

                        let stop_reason = if *any_tool_calls.lock().unwrap() {
                            StopReason::ToolUse
                        } else {
                            StopReason::EndTurn
                        };
                        events.push(Ok(StreamEvent::MessageStop { stop_reason }));
                    }

                    events
                }
            };

            futures::stream::iter(events)
        });

        Ok(Box::pin(mapped))
    }

    fn name(&self) -> &'static str {
        "Ollama"
    }

    async fn available_models(&self) -> Result<Vec<ModelInfo>, BackendError> {
        let models = self
            .client
            .list_local_models()
            .await
            .map_err(|err| map_ollama_error(err, &self.base_url_for_diagnostics))?;

        Ok(models.into_iter().map(local_model_to_model_info).collect())
    }

    fn default_model(&self) -> &ModelInfo {
        self.cached_default_model
            .get()
            .unwrap_or(&self.placeholder_model)
    }
}

/// Convert a sequence of clareon `Message`s into the `ChatMessage` list
/// expected by `ollama-rs`.
///
/// System prompt, if present, is prepended as a `System` role message.
/// `ToolUse` blocks in assistant messages become structured `tool_calls`.
/// `ToolResult` blocks (which clareon stores on user-role messages) become
/// `Tool` role messages whose content is the concatenated text of each
/// `ToolResultContent::Text` block.
fn convert_messages(system_prompt: &Option<String>, messages: &[Message]) -> Vec<ChatMessage> {
    let mut result = Vec::with_capacity(messages.len() + 1);

    if let Some(sys) = system_prompt {
        result.push(ChatMessage::new(MessageRole::System, sys.clone()));
    }

    for msg in messages {
        let mut tool_result_msgs: Vec<ChatMessage> = Vec::new();
        let mut text_parts: Vec<String> = Vec::new();
        let mut images: Vec<Image> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => text_parts.push(text.clone()),
                ContentBlock::Image { source } => {
                    let ImageSource::Base64 { data, .. } = source;
                    images.push(Image::from_base64(data));
                }
                ContentBlock::ToolUse { id: _, name, input } => {
                    tool_calls.push(ToolCall {
                        function: ToolCallFunction {
                            name: name.clone(),
                            arguments: input.clone(),
                        },
                    });
                }
                ContentBlock::ToolResult { content, .. } => {
                    let text = content
                        .iter()
                        .map(|c| match c {
                            ToolResultContent::Text { text } => text.clone(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    tool_result_msgs.push(ChatMessage::new(MessageRole::Tool, text));
                }
            }
        }

        let has_main_content =
            !text_parts.is_empty() || !images.is_empty() || !tool_calls.is_empty();
        if has_main_content {
            let role = match msg.role {
                Role::User => MessageRole::User,
                Role::Assistant => MessageRole::Assistant,
            };
            let mut chat_msg = ChatMessage::new(role, text_parts.join("\n"));
            if !images.is_empty() {
                chat_msg = chat_msg.with_images(images);
            }
            chat_msg.tool_calls = tool_calls;
            result.push(chat_msg);
        }

        result.extend(tool_result_msgs);
    }

    result
}

/// Convert an Ollama assistant `ChatMessage` plus usage counters into our
/// `Message`, plus the derived `StopReason` and `Usage`.
fn convert_response(
    response: ollama_rs::generation::chat::ChatMessage,
    model: &str,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    conversation_id: ConversationId,
) -> (Message, StopReason, Usage) {
    let usage = Usage {
        input_tokens: prompt_eval_count.unwrap_or(0) as i64,
        output_tokens: eval_count.unwrap_or(0) as i64,
        cache_read_input_tokens: None,
        cache_write_input_tokens: None,
    };

    let mut content: Vec<ContentBlock> = Vec::new();
    if !response.content.is_empty() {
        content.push(ContentBlock::Text {
            text: response.content.clone(),
        });
    }
    let has_tools = !response.tool_calls.is_empty();
    for (idx, tc) in response.tool_calls.into_iter().enumerate() {
        content.push(ContentBlock::ToolUse {
            id: format!("call_{idx}"),
            name: tc.function.name,
            input: tc.function.arguments,
        });
    }

    let stop_reason = if has_tools {
        StopReason::ToolUse
    } else {
        StopReason::EndTurn
    };

    let text_content = if response.content.is_empty() {
        None
    } else {
        Some(response.content)
    };

    let message = Message {
        id: 0,
        conversation_id,
        created_at: chrono::Utc::now().timestamp(),
        role: Role::Assistant,
        text_content,
        content,
        input_tokens: Some(usage.input_tokens),
        output_tokens: Some(usage.output_tokens),
        model: Some(model.to_string()),
    };

    (message, stop_reason, usage)
}

/// Convert clareon `ToolDefinition`s into ollama-rs `ToolInfo`s.
fn convert_tools(tools: &[ToolDefinition]) -> Vec<ToolInfo> {
    tools
        .iter()
        .map(|t| ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: match serde_json::from_value::<schemars::Schema>(
                    t.input_schema.clone(),
                ) {
                    Ok(schema) => schema,
                    Err(err) => {
                        warn!(
                            tool = %t.name,
                            error = %err,
                            "Failed to deserialize tool input_schema for Ollama; using default empty schema"
                        );
                        schemars::Schema::default()
                    }
                },
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_base_url_defaults() {
        let (host, port, diag) = parse_base_url(None);
        assert_eq!(host, "http://localhost");
        assert_eq!(port, 11434);
        assert_eq!(diag, "http://localhost:11434");
    }

    #[test]
    fn parse_base_url_empty_string() {
        let (host, port, _) = parse_base_url(Some(""));
        assert_eq!(host, "http://localhost");
        assert_eq!(port, 11434);
    }

    #[test]
    fn parse_base_url_custom_host_and_port() {
        let (host, port, diag) = parse_base_url(Some("http://192.168.1.10:11500"));
        assert_eq!(host, "http://192.168.1.10");
        assert_eq!(port, 11500);
        assert_eq!(diag, "http://192.168.1.10:11500");
    }

    #[test]
    fn parse_base_url_https_default_port() {
        let (host, port, _) = parse_base_url(Some("https://ollama.example.com"));
        assert_eq!(host, "https://ollama.example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn parse_base_url_malformed_falls_back_to_defaults() {
        let (host, port, _) = parse_base_url(Some("not a url"));
        assert_eq!(host, "http://localhost");
        assert_eq!(port, 11434);
    }

    #[test]
    fn from_config_uses_defaults_when_unset() {
        let cfg = OllamaConfig::default();
        let backend = OllamaBackend::from_config(&cfg);
        assert_eq!(backend.name(), "Ollama");
        assert_eq!(backend.default_model().id, "");
    }

    #[test]
    fn from_config_stores_configured_default_model() {
        let cfg = OllamaConfig {
            base_url: None,
            default_model: Some("llama3.2:3b".to_string()),
        };
        let backend = OllamaBackend::from_config(&cfg);
        assert_eq!(
            backend.configured_default_model.as_deref(),
            Some("llama3.2:3b")
        );
    }

    #[test]
    fn parse_owner_simple_name() {
        assert_eq!(parse_owner("llama3.2"), None);
        assert_eq!(parse_owner("llama3.2:3b"), None);
    }

    #[test]
    fn parse_owner_with_namespace() {
        assert_eq!(parse_owner("library/llama3.2"), Some("library".to_string()));
        assert_eq!(
            parse_owner("hf.co/user/model:tag"),
            Some("hf.co".to_string())
        );
    }

    #[test]
    fn local_model_to_model_info_basic() {
        use ollama_rs::models::LocalModel;
        let local = LocalModel {
            name: "llama3.2:3b".to_string(),
            modified_at: "2026-01-01T00:00:00Z".to_string(),
            size: 2_000_000_000,
        };
        let info = local_model_to_model_info(local);
        assert_eq!(info.id, "llama3.2:3b");
        assert_eq!(info.name, "llama3.2:3b");
        assert_eq!(info.context_window, 0);
        assert!(info.pricing.is_none());
        assert_eq!(info.owner, None);
    }

    #[test]
    fn local_model_to_model_info_with_namespace() {
        use ollama_rs::models::LocalModel;
        let local = LocalModel {
            name: "library/qwen2.5:7b".to_string(),
            modified_at: "2026-01-01T00:00:00Z".to_string(),
            size: 5_000_000_000,
        };
        let info = local_model_to_model_info(local);
        assert_eq!(info.owner, Some("library".to_string()));
    }

    #[test]
    fn select_default_model_configured_present() {
        let models = vec![
            ModelInfo {
                id: "a".to_string(),
                ..placeholder_model_info()
            },
            ModelInfo {
                id: "llama3.2:3b".to_string(),
                ..placeholder_model_info()
            },
        ];
        let chosen = select_default_model(Some("llama3.2:3b"), &models).unwrap();
        assert_eq!(chosen.id, "llama3.2:3b");
    }

    #[test]
    fn select_default_model_configured_absent() {
        let models = vec![ModelInfo {
            id: "a".to_string(),
            ..placeholder_model_info()
        }];
        let err = select_default_model(Some("missing"), &models).unwrap_err();
        match err {
            BackendError::ModelNotAvailable(name) => assert_eq!(name, "missing"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn select_default_model_unconfigured_uses_first() {
        let models = vec![
            ModelInfo {
                id: "first".to_string(),
                ..placeholder_model_info()
            },
            ModelInfo {
                id: "second".to_string(),
                ..placeholder_model_info()
            },
        ];
        let chosen = select_default_model(None, &models).unwrap();
        assert_eq!(chosen.id, "first");
    }

    #[test]
    fn select_default_model_unconfigured_empty_list() {
        let models: Vec<ModelInfo> = vec![];
        let err = select_default_model(None, &models).unwrap_err();
        match err {
            BackendError::ModelNotAvailable(msg) => {
                assert!(msg.contains("ollama pull"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // ---------- message / tool conversion helpers ----------

    use crate::types::{ContentBlock, ConversationId, Message, Role, ToolResultContent};

    fn user_text_message(text: &str) -> Message {
        Message {
            id: 1,
            conversation_id: ConversationId::from("c1"),
            created_at: 0,
            role: Role::User,
            text_content: Some(text.to_string()),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            input_tokens: None,
            output_tokens: None,
            model: None,
        }
    }

    fn assistant_tool_use_message(id: &str, name: &str, input: serde_json::Value) -> Message {
        Message {
            id: 2,
            conversation_id: ConversationId::from("c1"),
            created_at: 0,
            role: Role::Assistant,
            text_content: None,
            content: vec![ContentBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                input,
            }],
            input_tokens: None,
            output_tokens: None,
            model: None,
        }
    }

    fn user_tool_result_message(tool_use_id: &str, text: &str) -> Message {
        Message {
            id: 3,
            conversation_id: ConversationId::from("c1"),
            created_at: 0,
            role: Role::User,
            text_content: None,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: vec![ToolResultContent::Text {
                    text: text.to_string(),
                }],
                is_error: None,
            }],
            input_tokens: None,
            output_tokens: None,
            model: None,
        }
    }

    #[test]
    fn convert_messages_includes_system_prompt() {
        let messages = vec![user_text_message("hi")];
        let converted = convert_messages(&Some("You are helpful.".to_string()), &messages);
        assert_eq!(converted.len(), 2);
        assert!(matches!(
            converted[0].role,
            ollama_rs::generation::chat::MessageRole::System
        ));
        assert_eq!(converted[0].content, "You are helpful.");
        assert!(matches!(
            converted[1].role,
            ollama_rs::generation::chat::MessageRole::User
        ));
        assert_eq!(converted[1].content, "hi");
    }

    #[test]
    fn convert_messages_omits_system_when_none() {
        let messages = vec![user_text_message("hi")];
        let converted = convert_messages(&None, &messages);
        assert_eq!(converted.len(), 1);
    }

    #[test]
    fn convert_messages_assistant_tool_use() {
        let messages = vec![assistant_tool_use_message(
            "call_1",
            "read_file",
            serde_json::json!({"path": "/tmp/a"}),
        )];
        let converted = convert_messages(&None, &messages);
        assert_eq!(converted.len(), 1);
        assert!(matches!(
            converted[0].role,
            ollama_rs::generation::chat::MessageRole::Assistant
        ));
        assert_eq!(converted[0].tool_calls.len(), 1);
        assert_eq!(converted[0].tool_calls[0].function.name, "read_file");
    }

    #[test]
    fn convert_messages_tool_result_becomes_tool_role() {
        let messages = vec![user_tool_result_message("call_1", "file contents")];
        let converted = convert_messages(&None, &messages);
        assert_eq!(converted.len(), 1);
        assert!(matches!(
            converted[0].role,
            ollama_rs::generation::chat::MessageRole::Tool
        ));
        assert_eq!(converted[0].content, "file contents");
    }

    #[test]
    fn convert_tools_maps_definition() {
        use crate::backend::ToolDefinition;
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file from disk".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }];
        let converted = convert_tools(&tools);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].function.name, "read_file");
        assert_eq!(converted[0].function.description, "Read a file from disk");
    }

    #[test]
    fn convert_tools_empty() {
        let converted = convert_tools(&[]);
        assert!(converted.is_empty());
    }

    #[test]
    fn convert_messages_assistant_mixed_text_and_tool_use() {
        let msg = Message {
            id: 0,
            conversation_id: ConversationId::from("c1"),
            created_at: 0,
            role: Role::Assistant,
            text_content: Some("I'll read that for you.".to_string()),
            content: vec![
                ContentBlock::Text {
                    text: "I'll read that for you.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "/tmp/a"}),
                },
            ],
            input_tokens: None,
            output_tokens: None,
            model: None,
        };
        let converted = convert_messages(&None, &[msg]);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].content, "I'll read that for you.");
        assert_eq!(converted[0].tool_calls.len(), 1);
        assert_eq!(converted[0].tool_calls[0].function.name, "read_file");
        assert!(matches!(
            converted[0].role,
            ollama_rs::generation::chat::MessageRole::Assistant
        ));
    }

    #[test]
    fn convert_response_text_only() {
        use ollama_rs::generation::chat::{ChatMessage, MessageRole};

        let response_message = ChatMessage::new(MessageRole::Assistant, "Hello there".to_string());
        let conv_id = ConversationId::from("c1");
        let (message, stop_reason, usage) =
            convert_response(response_message, "llama3.2:3b", Some(10), Some(5), conv_id);
        assert_eq!(message.text_content.as_deref(), Some("Hello there"));
        assert!(matches!(stop_reason, crate::backend::StopReason::EndTurn));
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 5);
        assert_eq!(message.content.len(), 1);
        assert!(matches!(message.content[0], ContentBlock::Text { .. }));
    }

    #[test]
    fn convert_response_with_tool_calls() {
        use ollama_rs::generation::chat::{ChatMessage, MessageRole};
        use ollama_rs::generation::tools::{ToolCall, ToolCallFunction};

        let mut response_message = ChatMessage::new(MessageRole::Assistant, String::new());
        response_message.tool_calls = vec![ToolCall {
            function: ToolCallFunction {
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "/x"}),
            },
        }];

        let conv_id = ConversationId::from("c1");
        let (message, stop_reason, _usage) =
            convert_response(response_message, "llama3.2:3b", Some(0), Some(0), conv_id);
        assert!(matches!(stop_reason, crate::backend::StopReason::ToolUse));
        assert_eq!(message.content.len(), 1);
        assert!(matches!(
            &message.content[0],
            ContentBlock::ToolUse { name, .. } if name == "read_file"
        ));
    }

    /// Smoke test against a real Ollama daemon.
    ///
    /// Run with: `cargo test -p clareon-core ollama::tests::live_smoke -- --ignored`
    /// Requires `ollama serve` to be running locally with at least one model pulled.
    #[tokio::test]
    #[ignore]
    async fn live_smoke() {
        let cfg = OllamaConfig::default();
        let backend = OllamaBackend::from_config(&cfg);

        let models = backend
            .available_models()
            .await
            .expect("available_models should succeed against a running daemon");
        assert!(
            !models.is_empty(),
            "no local models found — pull one with `ollama pull <model>` before running"
        );

        let default = backend
            .resolve_default_model()
            .await
            .expect("resolve_default_model should succeed");
        assert_eq!(default.id, models[0].id);
    }
}
