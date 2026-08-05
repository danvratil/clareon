// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration management
//!
//! This module handles loading and saving configuration,
//! as well as secure storage of API keys via the system keyring.

mod manager;
mod mcp_import;
mod secrets;
mod settings;

pub use manager::ConfigManager;
pub use mcp_import::{import_mcp_servers_json, merge_imported_servers};
pub use secrets::{ANTHROPIC_API_KEY, SecretStore};
pub use settings::{
    AnthropicConfig, Config, McpConfig, McpServerConfig, McpTransportConfig, OllamaConfig,
    OpenAiBackendConfig, Provider, SandboxModeConfig,
};
