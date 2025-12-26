//! Storage layer for conversations and messages
//!
//! This module provides SQLite-based persistence for conversations,
//! messages, and related data with FTS5 search support.

mod database;

pub use database::Storage;
