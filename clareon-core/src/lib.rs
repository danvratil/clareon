// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Clareon Core Library
//!
//! This crate provides the core functionality for the Clareon assistant,
//! including LLM backends, conversation management, storage, and configuration.

pub mod backend;
pub mod config;
pub mod conversation;
pub mod error;
pub mod logging;
pub mod mcp;
pub mod storage;
pub mod tools;
pub mod types;

pub use backend::{
    AnthropicBackend, BedrockBackend, ChatRequest, ChatResponse, LlmBackend, StopReason,
};
pub use config::{
    Config, ConfigManager, McpConfig, McpServerConfig, McpTransportConfig, SandboxModeConfig,
    SecretStore, import_mcp_servers_json, merge_imported_servers,
};
pub use conversation::{ConversationManager, ConversationSession, StreamUpdate};
pub use error::{Error, Result};
pub use mcp::{
    McpManager, McpPromptMessage, McpPromptRef, McpPromptResult, McpResourceRef, McpServerStatus,
    McpServerStatusInfo,
};
pub use storage::Storage;
pub use tools::{
    ArtifactManager, BubblewrapSandbox, ExecutionContext, NoneSandbox, PersistentWorkspace,
    Sandbox, SandboxMode, Tool, ToolError, ToolExecutor, ToolRegistry, ToolResult,
    WorkspaceManager, register_builtin_tools,
};
