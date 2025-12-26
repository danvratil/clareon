//! Conversation management
//!
//! This module provides high-level conversation management,
//! orchestrating the interaction between storage, backends, and title generation.

mod manager;
mod title;

pub use manager::ConversationManager;
pub use title::TitleGenerator;
