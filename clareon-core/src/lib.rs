//! Clareon Core Library
//!
//! This crate provides the core functionality for the Clareon Claude assistant,
//! including LLM backends, conversation management, storage, and configuration.

pub mod backend;
pub mod config;
pub mod conversation;
pub mod error;
pub mod storage;
pub mod types;

pub use backend::{
    AnthropicBackend, BedrockBackend, ChatRequest, ChatResponse, LlmBackend, StopReason,
};
pub use config::{Config, SecretStore};
pub use conversation::{ConversationManager, StreamUpdate};
pub use error::{Error, Result};
pub use storage::Storage;
