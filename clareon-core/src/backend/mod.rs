// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! LLM Backend implementations
//!
//! This module provides the trait definition and implementations for
//! different LLM backends (Anthropic API, AWS Bedrock).

mod anthropic;
mod bedrock;
mod traits;

use std::sync::Arc;

use crate::{Config, config::Backend};
pub use anthropic::AnthropicBackend;
pub use bedrock::BedrockBackend;
pub use traits::*;

/// Create an LLM backend instance based on the provided configuration
pub async fn create_backend_from_config(config: &Config) -> Result<Arc<dyn LlmBackend>, String> {
    match config.default_backend {
        Backend::Anthropic => {
            // For now, only support API key from environment variable
            // Keyring support can be added later
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

            Ok(Arc::new(AnthropicBackend::new(api_key)))
        }
        Backend::Bedrock => {
            let region = &config.backends.bedrock.region;
            let profile = config.backends.bedrock.profile.as_deref();

            let backend = if let Some(profile) = profile {
                BedrockBackend::with_profile(region.clone(), profile.to_string())
                    .await
                    .map_err(|e| format!("Failed to create Bedrock backend: {}", e))?
            } else {
                BedrockBackend::new(region.clone())
                    .await
                    .map_err(|e| format!("Failed to create Bedrock backend: {}", e))?
            };

            Ok(Arc::new(backend))
        }
    }
}
