// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the Anthropic backend using the mock server.
//!
//! These tests verify:
//! - Streaming response handling
//! - Tool use functionality
//! - Error handling for various API error conditions

use clareon_core::backend::{
    ChatRequest, ContentDelta, LlmBackend, StopReason, StreamEvent, ToolDefinition,
};
use clareon_core::error::BackendError;
use clareon_core::types::{ContentBlock, Message};
use futures::StreamExt;
use mock_anthropic::MockServer;

/// Helper to create a backend pointing to the mock server
fn create_backend(base_url: &str) -> clareon_core::backend::AnthropicBackend {
    clareon_core::backend::AnthropicBackend::with_base_url("test-api-key", base_url)
}

/// Helper to create a simple user message
fn create_user_message(text: &str) -> Message {
    Message::user("test-conversation", text)
}

// ============================================================================
// Non-Streaming Tests
// ============================================================================

mod non_streaming {
    use super::*;

    #[tokio::test]
    async fn test_basic_message_response() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("Hello, Claude!")],
            "claude-sonnet-4-5-20250929",
        );

        let response = backend.send_message(&request).await.unwrap();

        // Verify we got a valid response
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert!(response.usage.input_tokens > 0);
        assert!(response.usage.output_tokens > 0);

        // Verify the message contains text
        assert!(!response.message.content.is_empty());
        let text = response
            .message
            .content
            .iter()
            .filter_map(|b| b.as_text())
            .collect::<Vec<_>>()
            .join("");
        assert!(!text.is_empty());

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_with_system_prompt() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("What's 2+2?")],
            "claude-sonnet-4-5-20250929",
        )
        .with_system_prompt("You are a helpful math assistant.");

        let response = backend.send_message(&request).await.unwrap();

        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert!(!response.message.content.is_empty());

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_with_max_tokens() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("Tell me a story")],
            "claude-sonnet-4-5-20250929",
        )
        .with_max_tokens(100);

        let response = backend.send_message(&request).await.unwrap();

        assert_eq!(response.stop_reason, StopReason::EndTurn);

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_with_temperature() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("Be creative")],
            "claude-sonnet-4-5-20250929",
        )
        .with_temperature(0.7);

        let response = backend.send_message(&request).await.unwrap();

        assert_eq!(response.stop_reason, StopReason::EndTurn);

        server.shutdown().await;
    }
}

// ============================================================================
// Streaming Tests
// ============================================================================

mod streaming {
    use super::*;

    #[tokio::test]
    async fn test_streaming_accumulates_text() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("Hello, Claude!")],
            "claude-sonnet-4-5-20250929",
        );

        let mut stream = backend.send_message_stream(&request).await.unwrap();

        let mut accumulated_text = String::new();
        let mut got_content_block_start = false;
        let mut got_content_block_stop = false;
        let mut got_message_stop = false;
        let mut got_usage = false;

        while let Some(event) = stream.next().await {
            match event.unwrap() {
                StreamEvent::ContentBlockStart { index, block } => {
                    assert_eq!(index, 0);
                    if let ContentBlock::Text { text } = block {
                        accumulated_text.push_str(&text);
                    }
                    got_content_block_start = true;
                }
                StreamEvent::ContentBlockDelta { index, delta } => {
                    assert_eq!(index, 0);
                    if let ContentDelta::Text { text } = delta {
                        accumulated_text.push_str(&text);
                    }
                }
                StreamEvent::ContentBlockStop { index } => {
                    assert_eq!(index, 0);
                    got_content_block_stop = true;
                }
                StreamEvent::MessageStop { stop_reason } => {
                    assert_eq!(stop_reason, StopReason::EndTurn);
                    got_message_stop = true;
                }
                StreamEvent::Usage(usage) => {
                    assert!(usage.input_tokens > 0);
                    got_usage = true;
                }
            }
        }

        // Verify we received all expected events
        assert!(
            got_content_block_start,
            "Should have received ContentBlockStart"
        );
        assert!(
            got_content_block_stop,
            "Should have received ContentBlockStop"
        );
        assert!(got_message_stop, "Should have received MessageStop");
        assert!(got_usage, "Should have received Usage");

        // Verify text was accumulated
        assert!(!accumulated_text.is_empty(), "Should have accumulated text");
        // The mock server sends Lorem Ipsum text
        assert!(
            accumulated_text.contains("Lorem ipsum"),
            "Should contain Lorem ipsum text"
        );

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_streaming_with_system_prompt() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("Hello")],
            "claude-sonnet-4-5-20250929",
        )
        .with_system_prompt("You are a helpful assistant.");

        let mut stream = backend.send_message_stream(&request).await.unwrap();

        let mut event_count = 0;
        while let Some(event) = stream.next().await {
            event.unwrap(); // Should not error
            event_count += 1;
        }

        assert!(event_count > 0, "Should have received events");

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_streaming_multiple_messages_in_conversation() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Simulate a multi-turn conversation
        let request = ChatRequest::new(
            vec![
                create_user_message("Hello"),
                Message::assistant(
                    "test-conversation",
                    vec![ContentBlock::text("Hi there!")],
                    "claude-sonnet-4-5-20250929",
                    10,
                    5,
                ),
                create_user_message("How are you?"),
            ],
            "claude-sonnet-4-5-20250929",
        );

        let mut stream = backend.send_message_stream(&request).await.unwrap();

        let mut got_message_stop = false;
        while let Some(event) = stream.next().await {
            if let StreamEvent::MessageStop { .. } = event.unwrap() {
                got_message_stop = true;
            }
        }

        assert!(got_message_stop, "Should have received MessageStop");

        server.shutdown().await;
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_error() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Use the trigger phrase to simulate rate limiting
        let request = ChatRequest::new(
            vec![create_user_message("trigger rate limit")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::RateLimited { retry_after_secs } => {
                // The mock server sets retry-after to 60
                assert_eq!(retry_after_secs, Some(60));
            }
            other => panic!("Expected RateLimited error, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_rate_limit_error_streaming() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger rate limit")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message_stream(&request).await;

        match result {
            Err(BackendError::RateLimited { retry_after_secs }) => {
                assert_eq!(retry_after_secs, Some(60));
            }
            Err(other) => panic!("Expected RateLimited error, got: {:?}", other),
            Ok(_) => panic!("Expected error, got success"),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_authentication_error() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Use the trigger phrase to simulate authentication error
        let request = ChatRequest::new(
            vec![create_user_message("trigger auth error")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::Authentication(msg) => {
                assert!(msg.contains("authentication") || msg.contains("credentials"));
            }
            other => panic!("Expected Authentication error, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_authentication_error_streaming() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger auth error")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message_stream(&request).await;

        match result {
            Err(BackendError::Authentication(_)) => {}
            Err(other) => panic!("Expected Authentication error, got: {:?}", other),
            Ok(_) => panic!("Expected error, got success"),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_unavailable_error() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger server error")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::ServiceUnavailable => {}
            other => panic!("Expected ServiceUnavailable error, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_service_unavailable_error_streaming() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger server error")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message_stream(&request).await;

        match result {
            Err(BackendError::ServiceUnavailable) => {}
            Err(other) => panic!("Expected ServiceUnavailable error, got: {:?}", other),
            Ok(_) => panic!("Expected error, got success"),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_invalid_request_error() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger invalid request")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::Api { status, message } => {
                assert_eq!(status, 400);
                assert!(message.contains("Invalid") || message.contains("invalid"));
            }
            other => panic!("Expected Api error with status 400, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_context_length_error() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger context limit")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::Api { status, message } => {
                assert_eq!(status, 400);
                assert!(message.contains("context") || message.contains("token"));
            }
            other => panic!("Expected Api error with context message, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_internal_server_error() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("trigger internal error")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::ServiceUnavailable => {}
            other => panic!("Expected ServiceUnavailable error, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_missing_api_key() {
        let server = MockServer::start().await.unwrap();
        // Create backend with empty API key
        let backend = clareon_core::backend::AnthropicBackend::with_base_url("", server.base_url());

        let request = ChatRequest::new(
            vec![create_user_message("Hello")],
            "claude-sonnet-4-5-20250929",
        );

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::Authentication(_) => {}
            other => panic!("Expected Authentication error, got: {:?}", other),
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_invalid_model() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Use a model that doesn't exist
        let request = ChatRequest::new(vec![create_user_message("Hello")], "non-existent-model");

        let result = backend.send_message(&request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::Api { status, message } => {
                assert_eq!(status, 400);
                assert!(message.contains("not found") || message.contains("Model"));
            }
            other => panic!("Expected Api error for invalid model, got: {:?}", other),
        }

        server.shutdown().await;
    }
}

// ============================================================================
// Tool Use Tests
// ============================================================================

mod tool_use {
    use super::*;

    #[tokio::test]
    async fn test_request_with_tool_definitions() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Create a request with tool definitions
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    }
                },
                "required": ["path"]
            }),
        }];

        let mut request = ChatRequest::new(
            vec![create_user_message("Read the file /tmp/test.txt")],
            "claude-sonnet-4-5-20250929",
        );
        request.tools = tools;

        // The mock server doesn't return tool use, but we verify the request doesn't fail
        let response = backend.send_message(&request).await.unwrap();

        assert_eq!(response.stop_reason, StopReason::EndTurn);

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_request_with_multiple_tools() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let tools = vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read the contents of a file".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write contents to a file".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "list_directory".to_string(),
                description: "List contents of a directory".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
            },
        ];

        let mut request = ChatRequest::new(
            vec![create_user_message("What files are in /tmp?")],
            "claude-sonnet-4-5-20250929",
        );
        request.tools = tools;

        let response = backend.send_message(&request).await.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_streaming_with_tools() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        }];

        let mut request = ChatRequest::new(
            vec![create_user_message("Read /tmp/test.txt")],
            "claude-sonnet-4-5-20250929",
        );
        request.tools = tools;

        let mut stream = backend.send_message_stream(&request).await.unwrap();

        let mut got_message_stop = false;
        while let Some(event) = stream.next().await {
            if let StreamEvent::MessageStop { .. } = event.unwrap() {
                got_message_stop = true;
            }
        }

        assert!(got_message_stop, "Should have received MessageStop");

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_tool_use_response() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        }];

        // Use trigger phrase to get a tool use response
        let mut request = ChatRequest::new(
            vec![create_user_message("trigger tool use read_file")],
            "claude-sonnet-4-5-20250929",
        );
        request.tools = tools;

        let response = backend.send_message(&request).await.unwrap();

        // The mock server should return a tool use when triggered
        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert!(
            response.message.has_tool_use(),
            "Response should contain tool use"
        );

        // Verify the tool use block
        let tool_uses: Vec<_> = response.message.tool_uses().collect();
        assert_eq!(tool_uses.len(), 1);
        if let ContentBlock::ToolUse { name, input, .. } = &tool_uses[0] {
            assert_eq!(name, "read_file");
            assert!(input.get("path").is_some());
        } else {
            panic!("Expected ToolUse block");
        }

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_tool_result_in_conversation() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Simulate a conversation with tool result
        let messages = vec![
            create_user_message("Read /tmp/test.txt"),
            Message::assistant(
                "test-conversation",
                vec![ContentBlock::tool_use(
                    "tool_123",
                    "read_file",
                    serde_json::json!({"path": "/tmp/test.txt"}),
                )],
                "claude-sonnet-4-5-20250929",
                50,
                20,
            ),
            Message {
                id: 0,
                conversation_id: "test-conversation".into(),
                created_at: chrono::Utc::now().timestamp(),
                role: clareon_core::types::Role::User,
                text_content: None,
                content: vec![ContentBlock::tool_result(
                    "tool_123",
                    vec![clareon_core::types::ToolResultContent::text(
                        "File contents: Hello, World!",
                    )],
                    false,
                )],
                input_tokens: None,
                output_tokens: None,
                model: None,
            },
        ];

        let request = ChatRequest::new(messages, "claude-sonnet-4-5-20250929");

        let response = backend.send_message(&request).await.unwrap();

        // Should get a normal response after tool result
        assert_eq!(response.stop_reason, StopReason::EndTurn);

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_tool_error_result() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        // Simulate a conversation with tool error result
        let messages = vec![
            create_user_message("Read /nonexistent/file.txt"),
            Message::assistant(
                "test-conversation",
                vec![ContentBlock::tool_use(
                    "tool_456",
                    "read_file",
                    serde_json::json!({"path": "/nonexistent/file.txt"}),
                )],
                "claude-sonnet-4-5-20250929",
                50,
                20,
            ),
            Message {
                id: 0,
                conversation_id: "test-conversation".into(),
                created_at: chrono::Utc::now().timestamp(),
                role: clareon_core::types::Role::User,
                text_content: None,
                content: vec![ContentBlock::tool_result(
                    "tool_456",
                    vec![clareon_core::types::ToolResultContent::text(
                        "Error: File not found",
                    )],
                    true, // is_error = true
                )],
                input_tokens: None,
                output_tokens: None,
                model: None,
            },
        ];

        let request = ChatRequest::new(messages, "claude-sonnet-4-5-20250929");

        let response = backend.send_message(&request).await.unwrap();

        // Should get a normal response even with tool error
        assert_eq!(response.stop_reason, StopReason::EndTurn);

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_streaming_tool_use_response() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read file contents".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"]
            }),
        }];

        let mut request = ChatRequest::new(
            vec![create_user_message("trigger tool use read_file")],
            "claude-sonnet-4-5-20250929",
        );
        request.tools = tools;

        let mut stream = backend.send_message_stream(&request).await.unwrap();

        let mut got_tool_use_start = false;
        let mut got_tool_input_delta = false;
        let mut stop_reason = None;

        while let Some(event) = stream.next().await {
            match event.unwrap() {
                StreamEvent::ContentBlockStart { block, .. } => {
                    if block.is_tool_use() {
                        got_tool_use_start = true;
                    }
                }
                StreamEvent::ContentBlockDelta {
                    delta: ContentDelta::ToolInput { .. },
                    ..
                } => {
                    got_tool_input_delta = true;
                }
                StreamEvent::MessageStop { stop_reason: sr } => {
                    stop_reason = Some(sr);
                }
                _ => {}
            }
        }

        assert!(
            got_tool_use_start,
            "Should have received tool use block start"
        );
        assert!(
            got_tool_input_delta,
            "Should have received tool input delta"
        );
        assert_eq!(
            stop_reason,
            Some(StopReason::ToolUse),
            "Stop reason should be ToolUse"
        );

        server.shutdown().await;
    }
}

// ============================================================================
// Backend Info Tests
// ============================================================================

mod backend_info {
    use super::*;

    #[tokio::test]
    async fn test_backend_name() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        assert_eq!(backend.name(), "Anthropic API");

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_available_models() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let models = backend.available_models();
        assert!(!models.is_empty());

        // Check that expected models are present
        let model_ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(model_ids.iter().any(|id| id.contains("sonnet")));
        assert!(model_ids.iter().any(|id| id.contains("opus")));
        assert!(model_ids.iter().any(|id| id.contains("haiku")));

        server.shutdown().await;
    }

    #[tokio::test]
    async fn test_default_model() {
        let server = MockServer::start().await.unwrap();
        let backend = create_backend(&server.base_url());

        let default = backend.default_model();
        assert!(default.id.contains("sonnet"), "Default should be Sonnet");
        assert!(default.context_window > 0);
        assert!(default.max_output_tokens > 0);

        server.shutdown().await;
    }
}
