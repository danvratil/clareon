// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::Duration};
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
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStartData },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: Delta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaData,
        usage: Usage,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    #[allow(dead_code)]
    Ping,
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
                    .map(|b| match b {
                        ContentBlock::Text { text } => text.clone(),
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

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mock_anthropic=debug,tower_http=debug".into()),
        )
        .init();

    // Load models from models-api.json
    let models_json = include_str!("../models.json");
    let models: ModelsResponse =
        serde_json::from_str(models_json).expect("Failed to parse models-api.json");

    tracing::info!("Loaded {} models from models-api.json", models.data.len());

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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8081")
        .await
        .expect("Failed to bind to port 8080");

    tracing::info!("Mock Anthropic API server listening on http://127.0.0.1:8081");
    tracing::info!("Endpoints:");
    tracing::info!("  GET  http://127.0.0.1:8081/v1/models");
    tracing::info!("  POST http://127.0.0.1:8081/v1/messages");

    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}
