// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Qt/QML integration layer

use std::sync::{Arc, Mutex, OnceLock};

use clareon_core::types::{ConversationSummary, SearchResult};

// Type alias for conversations cache
type ConversationsCache = Arc<Mutex<Vec<ConversationSummary>>>;

// Type alias for search results cache
type SearchResultsCache = Arc<Mutex<Vec<SearchResult>>>;

// Global cached state
static CONVERSATIONS_CACHE: OnceLock<ConversationsCache> = OnceLock::new();
static SEARCH_RESULTS_CACHE: OnceLock<SearchResultsCache> = OnceLock::new();

/// Get or initialize the conversations cache
pub(crate) fn conversations_cache() -> ConversationsCache {
    CONVERSATIONS_CACHE
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

/// Get or initialize the search results cache
pub(crate) fn search_results_cache() -> SearchResultsCache {
    SEARCH_RESULTS_CACHE
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}
