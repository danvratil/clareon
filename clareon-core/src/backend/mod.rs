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

pub use anthropic::AnthropicBackend;
pub use bedrock::BedrockBackend;
pub use traits::*;
