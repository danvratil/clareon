// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration test demonstrating mock server usage
//!
//! This test shows how to use the mock-anthropic library in tests
//! to test the Anthropic backend without making real API calls.

use clareon_core::backend::{AnthropicBackend, ChatRequest, LlmBackend};
use clareon_core::types::{ConversationId, Message, Role};
use futures::StreamExt;
use mock_anthropic::MockServer;

#[tokio::test]
async fn test_anthropic_backend_with_mock_server() {
    // Start mock server on a random port
    let server = MockServer::start().await.unwrap();
    println!("Mock server started at {}", server.base_url());

    // Create Anthropic backend pointing to mock server
    let backend = AnthropicBackend::with_base_url("test-api-key", server.base_url());

    // Create a simple message
    let conv_id = ConversationId::new();
    let messages = vec![Message::user(conv_id, "Hello, Claude!")];

    // Build request
    let request = ChatRequest::new(messages, "claude-sonnet-4-20250514").with_max_tokens(1024);

    // Send message (non-streaming)
    let response = backend.send_message(&request).await.unwrap();

    // Verify response
    assert_eq!(response.message.role, Role::Assistant);
    assert!(!response.message.content.is_empty());
    assert!(response.usage.input_tokens > 0);
    assert!(response.usage.output_tokens > 0);

    // Shutdown server
    server.shutdown().await;
}

#[tokio::test]
async fn test_anthropic_backend_streaming_with_mock_server() {
    // Start mock server
    let server = MockServer::start().await.unwrap();

    // Create backend
    let backend = AnthropicBackend::with_base_url("test-api-key", server.base_url());

    // Create message
    let conv_id = ConversationId::new();
    let messages = vec![Message::user(conv_id, "Tell me a story")];

    // Build request
    let request = ChatRequest::new(messages, "claude-sonnet-4-20250514").with_max_tokens(1024);

    // Send streaming message
    let mut stream = backend.send_message_stream(&request).await.unwrap();

    // Collect stream events
    let mut event_count = 0;
    while let Some(event_result) = stream.next().await {
        event_count += 1;
        match event_result {
            Ok(event) => println!("Received event: {:?}", event),
            Err(e) => println!("Stream error: {}", e),
        }
    }

    // Verify we received events
    assert!(event_count > 0, "Should receive streaming events");

    // Shutdown server
    server.shutdown().await;
}

#[tokio::test]
async fn test_mock_server_error_triggers() {
    // Start mock server
    let server = MockServer::start().await.unwrap();

    // Create backend
    let backend = AnthropicBackend::with_base_url("test-api-key", server.base_url());

    // Test rate limit error
    let conv_id = ConversationId::new();
    let messages = vec![Message::user(conv_id.clone(), "trigger rate limit")];
    let request = ChatRequest::new(messages, "claude-sonnet-4-20250514").with_max_tokens(1024);

    let result = backend.send_message(&request).await;

    // Verify error response
    assert!(
        result.is_err(),
        "Should return error for rate limit trigger"
    );
    let err = result.unwrap_err();
    println!("Rate limit error: {}", err);

    // Test authentication error
    let messages = vec![Message::user(conv_id, "trigger auth error")];
    let request = ChatRequest::new(messages, "claude-sonnet-4-20250514").with_max_tokens(1024);

    let result = backend.send_message(&request).await;

    assert!(result.is_err(), "Should return error for auth trigger");
    let err = result.unwrap_err();
    println!("Auth error: {}", err);

    // Shutdown server
    server.shutdown().await;
}
