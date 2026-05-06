// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! LLM Backend implementations
//!
//! This module provides the trait definition and implementations for
//! different LLM backends (Anthropic API, AWS Bedrock).

mod anthropic;
mod bedrock;
mod openai;
mod openrouter;
mod traits;

use std::sync::Arc;

use crate::{Config, config::Provider};
pub use anthropic::AnthropicBackend;
pub use bedrock::BedrockBackend;
pub use openai::OpenAiBackend;
pub use openrouter::OpenRouterBackend;
pub use traits::*;

/// Create an LLM backend instance based on the provided configuration
///
/// Maps user-facing providers to internal backend implementations.
pub async fn create_backend_from_config(config: &Config) -> Result<Arc<dyn LlmBackend>, String> {
    match config.default_provider {
        Provider::OpenAi | Provider::LiteLlm => {
            let provider_config = match config.default_provider {
                Provider::OpenAi => &config.providers.openai,
                Provider::LiteLlm => &config.providers.litellm,
                _ => unreachable!(),
            };
            let backend = OpenAiBackend::from_config(provider_config);
            Ok(Arc::new(backend))
        }
        Provider::OpenRouter => {
            let backend = OpenRouterBackend::from_config(&config.providers.openrouter)
                .map_err(|e| format!("Failed to create OpenRouter backend: {}", e))?;
            Ok(Arc::new(backend))
        }
        Provider::Anthropic => {
            let backend = AnthropicBackend::from_config(&config.providers.anthropic)
                .await
                .map_err(|e| format!("Failed to create Anthropic backend: {}", e))?;

            Ok(Arc::new(backend))
        }
        Provider::Bedrock => {
            let region = &config.providers.bedrock.region;
            let profile = config.providers.bedrock.profile.as_deref();

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
