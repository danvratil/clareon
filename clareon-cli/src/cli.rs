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

    /// Resume a conversation by ID
    #[arg(long, value_name = "ID")]
    pub resume: Option<i64>,

    /// Search conversations
    #[arg(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Backend to use (bedrock or anthropic)
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,

    /// Model to use
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Initial prompt (starts conversation without TUI)
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,
}
