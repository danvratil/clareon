// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mock Anthropic API server for testing
//!
//! This library provides a mock server that implements the Anthropic API
//! for testing purposes. It can be spawned programmatically and will bind
//! to a random available port.
//!
//! # Example
//!
//! ```no_run
//! use mock_anthropic::MockServer;
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = MockServer::start().await.unwrap();
//!     println!("Mock server running at {}", server.base_url());
//!
//!     // Use the server...
//!
//!     server.shutdown().await;
//! }
//! ```

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::oneshot;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

const LOREM_IPSUM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

// ============================================================================
// API Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelInfo {
    #[serde(rename = "type")]
    type_: String,
    id: String,
    display_name: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
    has_more: bool,
    first_id: String,
    last_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    #[serde(default)]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: ContentOrString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ContentOrString {
    String(String),
    Array(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: serde_json::Value,
        #[serde(default)]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageResponse {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    role: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: String,
    stop_sequence: Option<String>,
    usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

// SSE Event types
// Note: ToolUseStart is handled separately in serialization
#[derive(Debug, Clone)]
enum StreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    /// Tool use content block start
    ToolUseStart {
        index: u32,
        id: String,
        name: String,
    },
    ContentBlockDelta {
        index: u32,
        delta: Delta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: Usage,
    },
    MessageStop,
    #[allow(dead_code)]
    Ping,
}

impl Serialize for StreamEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            StreamEvent::MessageStart { message } => {
                let mut state = serializer.serialize_struct("StreamEvent", 2)?;
                state.serialize_field("type", "message_start")?;
                state.serialize_field("message", message)?;
                state.end()
            }
            StreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let mut state = serializer.serialize_struct("StreamEvent", 3)?;
                state.serialize_field("type", "content_block_start")?;
                state.serialize_field("index", index)?;
                state.serialize_field("content_block", content_block)?;
                state.end()
            }
            StreamEvent::ToolUseStart { index, id, name } => {
                // Serialize as content_block_start with tool_use content block
                #[derive(Serialize)]
                struct ToolUseBlock<'a> {
                    #[serde(rename = "type")]
                    type_: &'static str,
                    id: &'a str,
                    name: &'a str,
                    input: serde_json::Value,
                }
                let mut state = serializer.serialize_struct("StreamEvent", 3)?;
                state.serialize_field("type", "content_block_start")?;
                state.serialize_field("index", index)?;
                state.serialize_field(
                    "content_block",
                    &ToolUseBlock {
                        type_: "tool_use",
                        id,
                        name,
                        input: serde_json::json!({}),
                    },
                )?;
                state.end()
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                let mut state = serializer.serialize_struct("StreamEvent", 3)?;
                state.serialize_field("type", "content_block_delta")?;
                state.serialize_field("index", index)?;
                state.serialize_field("delta", delta)?;
                state.end()
            }
            StreamEvent::ContentBlockStop { index } => {
                let mut state = serializer.serialize_struct("StreamEvent", 2)?;
                state.serialize_field("type", "content_block_stop")?;
                state.serialize_field("index", index)?;
                state.end()
            }
            StreamEvent::MessageDelta { delta, usage } => {
                let mut state = serializer.serialize_struct("StreamEvent", 3)?;
                state.serialize_field("type", "message_delta")?;
                state.serialize_field("delta", delta)?;
                state.serialize_field("usage", usage)?;
                state.end()
            }
            StreamEvent::MessageStop => {
                let mut state = serializer.serialize_struct("StreamEvent", 1)?;
                state.serialize_field("type", "message_stop")?;
                state.end()
            }
            StreamEvent::Ping => {
                let mut state = serializer.serialize_struct("StreamEvent", 1)?;
                state.serialize_field("type", "ping")?;
                state.end()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MessageStartData {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    role: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: Usage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Serialize)]
struct MessageDeltaData {
    stop_reason: String,
    stop_sequence: Option<String>,
}

// ============================================================================
// Server State
// ============================================================================

struct AppState {
    models: ModelsResponse,
}

// ============================================================================
// Handlers
// ============================================================================

async fn list_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(state.models.clone())
}

/// Extract user message text from the request
fn extract_user_message_text(request: &MessageRequest) -> String {
    request
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .filter_map(|m| match &m.content {
            ContentOrString::String(s) => Some(s.clone()),
            ContentOrString::Array(blocks) => {
                let text: Vec<String> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.clone()),
                        ContentBlock::ToolUse { .. } | ContentBlock::ToolResult { .. } => None,
                    })
                    .collect();
                if text.is_empty() {
                    None
                } else {
                    Some(text.join(" "))
                }
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
        .to_lowercase()
}

/// Check for tool use trigger in user message and return the tool name if found
fn check_tool_use_trigger(text: &str) -> Option<String> {
    if text.contains("trigger tool use") {
        // Extract tool name from "trigger tool use <tool_name>"
        let parts: Vec<&str> = text.split("trigger tool use").collect();
        if parts.len() > 1 {
            let tool_name = parts[1].split_whitespace().next().unwrap_or("read_file");
            return Some(tool_name.to_string());
        }
        return Some("read_file".to_string());
    }
    None
}

/// Check for error trigger keywords in user message and return appropriate error response
fn check_error_triggers(text: &str) -> Option<Response> {
    // Rate limit error
    if text.contains("trigger rate limit") || text.contains("trigger ratelimit") {
        return Some(
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", "60")],
                Json(serde_json::json!({
                    "error": {
                        "type": "rate_limit_error",
                        "message": "Rate limit exceeded. Please try again later."
                    }
                })),
            )
                .into_response(),
        );
    }

    // Server error (service unavailable)
    if text.contains("trigger server error") || text.contains("trigger service unavailable") {
        return Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": {
                        "type": "api_error",
                        "message": "Service temporarily unavailable"
                    }
                })),
            )
                .into_response(),
        );
    }

    // Internal server error
    if text.contains("trigger internal error") {
        return Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": {
                        "type": "api_error",
                        "message": "Internal server error occurred"
                    }
                })),
            )
                .into_response(),
        );
    }

    // Authentication error
    if text.contains("trigger auth error") || text.contains("trigger authentication") {
        return Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": {
                        "type": "authentication_error",
                        "message": "Invalid authentication credentials"
                    }
                })),
            )
                .into_response(),
        );
    }

    // Context length exceeded
    if text.contains("trigger context limit") || text.contains("trigger context length") {
        return Some(
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": "Request exceeds maximum context length of 200000 tokens"
                    }
                })),
            )
                .into_response(),
        );
    }

    // Invalid request error
    if text.contains("trigger invalid request") {
        return Some(
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": "Invalid request parameters"
                    }
                })),
            )
                .into_response(),
        );
    }

    None
}

async fn create_message(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<MessageRequest>,
) -> Response {
    // Validate API key
    if let Some(api_key) = headers.get("x-api-key") {
        if api_key.is_empty() {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": {
                        "type": "authentication_error",
                        "message": "Invalid API key"
                    }
                })),
            )
                .into_response();
        }
    } else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "type": "authentication_error",
                    "message": "Missing API key"
                }
            })),
        )
            .into_response();
    }

    // Validate model exists
    if !state.models.data.iter().any(|m| m.id == request.model) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": format!("Model '{}' not found", request.model)
                }
            })),
        )
            .into_response();
    }

    // Check for error triggers in user messages
    let user_message_text = extract_user_message_text(&request);
    if let Some(error_response) = check_error_triggers(&user_message_text) {
        return error_response;
    }

    let message_id = format!("msg_{}", Uuid::new_v4().simple());

    // Check for tool use trigger
    if let Some(tool_name) = check_tool_use_trigger(&user_message_text) {
        if request.stream {
            return create_streaming_tool_use_response(message_id, request, tool_name)
                .into_response();
        } else {
            return create_tool_use_response(message_id, request, tool_name).into_response();
        }
    }

    if request.stream {
        create_streaming_response(message_id, request).into_response()
    } else {
        create_non_streaming_response(message_id, request).into_response()
    }
}

fn create_non_streaming_response(
    message_id: String,
    request: MessageRequest,
) -> Json<MessageResponse> {
    let response = MessageResponse {
        id: message_id,
        type_: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![ContentBlock::Text {
            text: LOREM_IPSUM.to_string(),
        }],
        model: request.model,
        stop_reason: "end_turn".to_string(),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 100,
            output_tokens: 50,
        },
    };

    Json(response)
}

fn create_streaming_response(
    message_id: String,
    request: MessageRequest,
) -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let words: Vec<&str> = LOREM_IPSUM.split_whitespace().collect();
    let model = request.model.clone();

    let stream = stream::iter(0..words.len() + 5)
        .enumerate()
        .then(move |(_idx, i)| {
            let message_id = message_id.clone();
            let model = model.clone();
            let words = words.clone();

            async move {
                if i > 0 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }

                let event = if i == 0 {
                    // message_start
                    StreamEvent::MessageStart {
                        message: MessageStartData {
                            id: message_id.clone(),
                            type_: "message".to_string(),
                            role: "assistant".to_string(),
                            content: vec![],
                            model: model.clone(),
                            stop_reason: None,
                            stop_sequence: None,
                            usage: Usage {
                                input_tokens: 100,
                                output_tokens: 0,
                            },
                        },
                    }
                } else if i == 1 {
                    // content_block_start
                    StreamEvent::ContentBlockStart {
                        index: 0,
                        content_block: ContentBlock::Text {
                            text: String::new(),
                        },
                    }
                } else if i < words.len() + 2 {
                    // content_block_delta
                    let word_idx = i - 2;
                    let text = if word_idx == 0 {
                        words[word_idx].to_string()
                    } else {
                        format!(" {}", words[word_idx])
                    };
                    StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: Delta::TextDelta { text },
                    }
                } else if i == words.len() + 2 {
                    // content_block_stop
                    StreamEvent::ContentBlockStop { index: 0 }
                } else if i == words.len() + 3 {
                    // message_delta
                    StreamEvent::MessageDelta {
                        delta: MessageDeltaData {
                            stop_reason: "end_turn".to_string(),
                            stop_sequence: None,
                        },
                        usage: Usage {
                            input_tokens: 100,
                            output_tokens: words.len() as u32,
                        },
                    }
                } else {
                    // message_stop
                    StreamEvent::MessageStop
                };

                let event_type = match &event {
                    StreamEvent::MessageStart { .. } => "message_start",
                    StreamEvent::ContentBlockStart { .. } => "content_block_start",
                    StreamEvent::ToolUseStart { .. } => "content_block_start",
                    StreamEvent::ContentBlockDelta { .. } => "content_block_delta",
                    StreamEvent::ContentBlockStop { .. } => "content_block_stop",
                    StreamEvent::MessageDelta { .. } => "message_delta",
                    StreamEvent::MessageStop => "message_stop",
                    StreamEvent::Ping => "ping",
                };

                let data = serde_json::to_string(&event).unwrap();

                Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event(event_type)
                        .data(data),
                )
            }
        });

    Sse::new(stream)
}

fn create_tool_use_response(
    message_id: String,
    request: MessageRequest,
    tool_name: String,
) -> Json<MessageResponse> {
    let tool_id = format!("toolu_{}", Uuid::new_v4().simple());
    let input = generate_tool_input(&tool_name);

    let response = MessageResponse {
        id: message_id,
        type_: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![ContentBlock::ToolUse {
            id: tool_id,
            name: tool_name,
            input,
        }],
        model: request.model,
        stop_reason: "tool_use".to_string(),
        stop_sequence: None,
        usage: Usage {
            input_tokens: 100,
            output_tokens: 30,
        },
    };

    Json(response)
}

fn create_streaming_tool_use_response(
    message_id: String,
    request: MessageRequest,
    tool_name: String,
) -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let model = request.model.clone();
    let tool_id = format!("toolu_{}", Uuid::new_v4().simple());
    let input = generate_tool_input(&tool_name);
    let input_json = serde_json::to_string(&input).unwrap();

    // Split input JSON into chunks for streaming
    let chunks: Vec<String> = input_json
        .chars()
        .collect::<Vec<_>>()
        .chunks(10)
        .map(|c| c.iter().collect())
        .collect();
    let num_chunks = chunks.len();

    let stream = stream::iter(0..num_chunks + 5)
        .enumerate()
        .then(move |(_idx, i)| {
            let message_id = message_id.clone();
            let model = model.clone();
            let tool_id = tool_id.clone();
            let tool_name = tool_name.clone();
            let chunks = chunks.clone();

            async move {
                if i > 0 {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }

                let event = if i == 0 {
                    // message_start
                    StreamEvent::MessageStart {
                        message: MessageStartData {
                            id: message_id.clone(),
                            type_: "message".to_string(),
                            role: "assistant".to_string(),
                            content: vec![],
                            model: model.clone(),
                            stop_reason: None,
                            stop_sequence: None,
                            usage: Usage {
                                input_tokens: 100,
                                output_tokens: 0,
                            },
                        },
                    }
                } else if i == 1 {
                    // content_block_start for tool_use
                    StreamEvent::ToolUseStart {
                        index: 0,
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                    }
                } else if i < num_chunks + 2 {
                    // content_block_delta with partial JSON
                    let chunk_idx = i - 2;
                    StreamEvent::ContentBlockDelta {
                        index: 0,
                        delta: Delta::InputJsonDelta {
                            partial_json: chunks[chunk_idx].clone(),
                        },
                    }
                } else if i == num_chunks + 2 {
                    // content_block_stop
                    StreamEvent::ContentBlockStop { index: 0 }
                } else if i == num_chunks + 3 {
                    // message_delta
                    StreamEvent::MessageDelta {
                        delta: MessageDeltaData {
                            stop_reason: "tool_use".to_string(),
                            stop_sequence: None,
                        },
                        usage: Usage {
                            input_tokens: 100,
                            output_tokens: 30,
                        },
                    }
                } else {
                    // message_stop
                    StreamEvent::MessageStop
                };

                let event_type = match &event {
                    StreamEvent::MessageStart { .. } => "message_start",
                    StreamEvent::ContentBlockStart { .. } => "content_block_start",
                    StreamEvent::ToolUseStart { .. } => "content_block_start",
                    StreamEvent::ContentBlockDelta { .. } => "content_block_delta",
                    StreamEvent::ContentBlockStop { .. } => "content_block_stop",
                    StreamEvent::MessageDelta { .. } => "message_delta",
                    StreamEvent::MessageStop => "message_stop",
                    StreamEvent::Ping => "ping",
                };

                let data = serde_json::to_string(&event).unwrap();

                Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event(event_type)
                        .data(data),
                )
            }
        });

    Sse::new(stream)
}

/// Generate mock input for a tool based on its name
fn generate_tool_input(tool_name: &str) -> serde_json::Value {
    match tool_name {
        "read_file" => serde_json::json!({
            "path": "/tmp/test.txt"
        }),
        "write_file" => serde_json::json!({
            "path": "/tmp/output.txt",
            "content": "Hello, World!"
        }),
        "list_directory" => serde_json::json!({
            "path": "/tmp"
        }),
        _ => serde_json::json!({
            "input": "mock_value"
        }),
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Mock Anthropic API server that can be spawned for testing
pub struct MockServer {
    port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl MockServer {
    /// Start a new mock server on a random available port
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mock_anthropic::MockServer;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let server = MockServer::start().await.unwrap();
    ///     println!("Server running at {}", server.base_url());
    /// }
    /// ```
    pub async fn start() -> Result<Self, std::io::Error> {
        Self::start_with_port(0).await
    }

    /// Start a new mock server on a specific port (use 0 for random)
    pub async fn start_with_port(port: u16) -> Result<Self, std::io::Error> {
        // Load models from models.json
        let models_json = include_str!("../models.json");
        let models: ModelsResponse =
            serde_json::from_str(models_json).expect("Failed to parse models.json");

        let state = Arc::new(AppState { models });

        // Configure CORS
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Build router
        let app = Router::new()
            .route("/v1/models", get(list_models))
            .route("/v1/messages", post(create_message))
            .layer(cors)
            .layer(tower_http::trace::TraceLayer::new_for_http())
            .with_state(state);

        // Bind to port (0 = random available port)
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        let addr = listener.local_addr()?;
        let actual_port = addr.port();

        tracing::debug!("Mock Anthropic API server bound to {}", addr);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Spawn server in background
        let server_handle = tokio::spawn(async move {
            let server = axum::serve(listener, app);

            // Run server with graceful shutdown
            let graceful = server.with_graceful_shutdown(async {
                shutdown_rx.await.ok();
                tracing::debug!("Mock server shutting down gracefully");
            });

            if let Err(e) = graceful.await {
                tracing::error!("Mock server error: {}", e);
            }
        });

        Ok(Self {
            port: actual_port,
            shutdown_tx: Some(shutdown_tx),
            server_handle: Some(server_handle),
        })
    }

    /// Get the port the server is running on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the base URL for the mock server
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Get the socket address the server is bound to
    pub fn addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(handle) = self.server_handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        // Send shutdown signal if not already sent
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_starts_and_stops() {
        let server = MockServer::start().await.unwrap();
        assert!(server.port() > 0);
        assert!(server.base_url().starts_with("http://127.0.0.1:"));
        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_mock_server_responds_to_requests() {
        let server = MockServer::start().await.unwrap();
        let base_url = server.base_url();

        // Test models endpoint
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/v1/models", base_url))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        let models: ModelsResponse = response.json().await.unwrap();
        assert!(!models.data.is_empty());

        server.shutdown().await;
    }
}
