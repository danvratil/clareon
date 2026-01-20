// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Storage layer for conversations and messages
//!
//! This module provides SQLite-based persistence for conversations,
//! messages, and related data with FTS5 search support.
//!
//! The storage layer is organized into domain-specific modules:
//! - `database`: Core database connection and initialization
//! - `conversations`: Conversation CRUD operations
//! - `messages`: Message CRUD, user files, and full-text search
//! - `artifacts`: Artifact tracking and management
//! - `workspaces`: Workspace metadata operations

mod artifacts;
mod conversations;
mod database;
mod messages;
mod workspaces;

pub use database::Storage;
