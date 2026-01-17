// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error types for Clareon Core

use thiserror::Error;

use crate::types::ConversationId;

/// Result type alias using the Clareon error type
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Clareon Core
#[derive(Debug, Error)]
pub enum Error {
    // Storage errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Conversation not found: {0}")]
    ConversationNotFound(ConversationId),

    #[error("Message not found: {0}")]
    MessageNotFound(i64),

    // Backend errors
    #[error("Backend error: {0}")]
    Backend(#[from] BackendError),

    // Configuration errors
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    // Tool errors
    #[error("Tool error: {0}")]
    Tool(#[from] crate::tools::ToolError),

    // Serialization errors
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors specific to LLM backends
#[derive(Debug, Error)]
pub enum BackendError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("AWS SDK error: {0}")]
    AwsSdk(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Rate limited, retry after {retry_after_secs:?} seconds")]
    RateLimited { retry_after_secs: Option<u64> },

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Model not available: {0}")]
    ModelNotAvailable(String),

    #[error("Context length exceeded: max {max_tokens} tokens")]
    ContextLengthExceeded { max_tokens: u64 },

    #[error("Request timeout")]
    Timeout,

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Errors specific to configuration
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(std::path::PathBuf),

    #[error("Invalid config: {0}")]
    Invalid(String),

    #[error("Failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Failed to read/write config: {0}")]
    Io(#[from] std::io::Error),

    #[error("Secret service error: {0}")]
    SecretService(String),

    #[error("Secret not found: {0}")]
    SecretNotFound(String),
}
