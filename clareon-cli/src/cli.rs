// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command line argument parsing

use clap::Parser;

/// Clareon - Claude assistant for Linux
#[derive(Debug, Parser)]
#[command(name = "clareon")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// List past conversations
    #[arg(long)]
    pub chats: bool,

    /// Resume a conversation by ID (UUID)
    #[arg(long, value_name = "UUID")]
    pub resume: Option<String>,

    /// Search conversations
    #[arg(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Backend to use (bedrock or anthropic)
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,

    /// Model to use
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// AWS profile to use (for Bedrock backend)
    #[arg(long, value_name = "PROFILE")]
    pub profile: Option<String>,

    /// AWS region to use (for Bedrock backend)
    #[arg(long, value_name = "REGION")]
    pub region: Option<String>,

    /// Initial prompt (starts conversation without TUI)
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,
}
