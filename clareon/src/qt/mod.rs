// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Qt/QML integration layer

use std::sync::{Arc, Mutex, OnceLock};

use clareon_core::types::{ConversationSummary, SearchResult};

use crate::service::ModelInfoData;

// Re-export from service_controller
pub use crate::service_controller::init_service_handle;

// Type alias for conversations cache
type ConversationsCache = Arc<Mutex<Vec<ConversationSummary>>>;

// Type alias for search results cache
type SearchResultsCache = Arc<Mutex<Vec<SearchResult>>>;

// Type alias for models cache
type ModelsCache = Arc<Mutex<Vec<ModelInfoData>>>;

// Global cached state
static CONVERSATIONS_CACHE: OnceLock<ConversationsCache> = OnceLock::new();
static SEARCH_RESULTS_CACHE: OnceLock<SearchResultsCache> = OnceLock::new();
static MODELS_CACHE: OnceLock<ModelsCache> = OnceLock::new();

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

/// Get or initialize the models cache
pub(crate) fn models_cache() -> ModelsCache {
    MODELS_CACHE
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}
