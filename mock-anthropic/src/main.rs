// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mock Anthropic API server binary
//!
//! This binary starts the mock server on port 8081 for manual testing.
//! For programmatic use in tests, use the `mock_anthropic` library instead.

use mock_anthropic::MockServer;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mock_anthropic=debug,tower_http=debug".into()),
        )
        .init();

    // Start server on port 8081
    let server = MockServer::start_with_port(8081)
        .await
        .expect("Failed to start mock server");

    tracing::info!(
        "Mock Anthropic API server listening on {}",
        server.base_url()
    );
    tracing::info!("Endpoints:");
    tracing::info!("  GET  {}/v1/models", server.base_url());
    tracing::info!("  POST {}/v1/messages", server.base_url());
    tracing::info!("Press Ctrl+C to stop");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");

    tracing::info!("Shutting down...");
    server.shutdown().await;
}
