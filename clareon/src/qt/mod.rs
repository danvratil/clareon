// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Qt/QML integration layer

use std::sync::{Arc, Mutex, OnceLock};

use clareon_core::types::ConversationSummary;

// Re-export from service_controller
pub use crate::service_controller::init_service_handle;

// Type alias for conversations cache
type ConversationsCache = Arc<Mutex<Vec<ConversationSummary>>>;

// Global cached state
static CONVERSATIONS_CACHE: OnceLock<ConversationsCache> = OnceLock::new();

/// Get or initialize the conversations cache
pub(crate) fn conversations_cache() -> ConversationsCache {
    CONVERSATIONS_CACHE
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}
