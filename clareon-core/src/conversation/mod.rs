// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conversation management
//!
//! This module provides high-level conversation management,
//! orchestrating the interaction between storage, backends, and title generation.

mod manager;
mod session;
mod title;

pub use manager::{
    ConversationManager, PendingToolUse, StreamUpdate, ToolApprovalDecision, ToolExecutionStatus,
};
pub use session::ConversationSession;
pub use title::TitleGenerator;
