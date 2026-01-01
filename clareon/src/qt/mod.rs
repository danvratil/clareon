// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Qt/QML integration layer

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc;

use clareon_core::types::{ConversationId, ConversationSummary};

use crate::service::{MessageData, Response};

// Re-export from service_controller
pub use crate::service_controller::init_service_handle;

// Type aliases for complex cache types
type ConversationsCache = Arc<Mutex<Vec<ConversationSummary>>>;
type MessagesCache = Arc<Mutex<HashMap<ConversationId, Vec<MessageData>>>>;

// Global response receiver
static RESPONSE_RX: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<Response>>>> = OnceLock::new();

// Global cached state (shared between ServiceController and models)
static CONVERSATIONS_CACHE: OnceLock<ConversationsCache> = OnceLock::new();
static MESSAGES_CACHE: OnceLock<MessagesCache> = OnceLock::new();

/// Initialize the response receiver (must be called before Qt starts)
pub fn init_response_receiver(rx: mpsc::UnboundedReceiver<Response>) {
    RESPONSE_RX.set(Mutex::new(Some(rx))).ok();
}

/// Take the response receiver (can only be called once, during ServiceController initialization)
pub(crate) fn take_response_receiver() -> mpsc::UnboundedReceiver<Response> {
    RESPONSE_RX
        .get()
        .expect("Response receiver not initialized")
        .lock()
        .unwrap()
        .take()
        .expect("Response receiver already taken")
}

/// Get or initialize the conversations cache
pub(crate) fn conversations_cache() -> ConversationsCache {
    CONVERSATIONS_CACHE
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

/// Get or initialize the messages cache
pub(crate) fn messages_cache() -> MessagesCache {
    MESSAGES_CACHE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}
