// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configuration management
//!
//! This module handles loading and saving configuration,
//! as well as secure storage of API keys via the system keyring.

mod manager;
mod secrets;
mod settings;

pub use manager::ConfigManager;
pub use secrets::{ANTHROPIC_API_KEY, SecretStore};
pub use settings::{AnthropicConfig, Backend, Config, SandboxModeConfig};
