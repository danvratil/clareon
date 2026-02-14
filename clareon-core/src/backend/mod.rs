// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
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

use crate::config::ProfileId;
use crate::{Config, config::Backend};
pub use anthropic::AnthropicBackend;
pub use bedrock::BedrockBackend;
pub use traits::*;

/// Create an LLM backend instance based on the provided configuration
pub async fn create_backend_from_config(
    config: &Config,
    profile_id: &ProfileId,
) -> Result<Arc<dyn LlmBackend>, String> {
    match config.default_backend {
        Backend::Anthropic => {
            let backend = AnthropicBackend::from_config(&config.backends.anthropic, profile_id)
                .await
                .map_err(|e| format!("Failed to create Anthropic backend: {}", e))?;

            Ok(Arc::new(backend))
        }
        Backend::Bedrock => {
            let region = &config.backends.bedrock.region;
            let aws_profile = config.backends.bedrock.profile.as_deref();

            let backend = if let Some(aws_profile) = aws_profile {
                BedrockBackend::with_profile(region.clone(), aws_profile.to_string())
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
