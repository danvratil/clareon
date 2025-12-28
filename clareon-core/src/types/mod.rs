// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core data types for Clareon

mod content;
mod conversation;
mod message;
mod workspace;

pub use content::{ContentBlock, ToolResultContent};
pub use conversation::{Conversation, ConversationId, ConversationSummary, SearchResult};
pub use message::{Message, Role};
pub use workspace::{Artifact, UserFile, WorkspaceMetadata};
