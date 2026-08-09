// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Model Context Protocol (MCP) client integration.

mod content;
mod manager;
mod names;
mod oauth;
mod tool_adapter;

pub use content::{flatten_content_blocks, flatten_prompt_messages, flatten_resource_contents};
pub use manager::{
    McpManager, McpPromptArgRef, McpPromptMessage, McpPromptRef, McpPromptResult, McpResourceRef,
    McpServerStatus, McpServerStatusInfo,
};
pub use names::prefixed_tool_name;
pub use oauth::{PendingOAuthLogin, clear_oauth_tokens, oauth_logged_in, open_in_browser};
