// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! ServiceController - Qt bridge to the service layer

use std::pin::Pin;
use std::sync::OnceLock;

use clareon_core::types::ConversationId;
use cxx_qt::Threading;

use crate::service::{Command, Response, ServiceHandle};

// Global service handle (set during initialization)
static SERVICE_HANDLE: OnceLock<ServiceHandle> = OnceLock::new();

/// Initialize the global service handle
pub fn init_service_handle(handle: ServiceHandle) {
    SERVICE_HANDLE.set(handle).ok();
}

/// Get the global service handle
fn get_service_handle() -> ServiceHandle {
    SERVICE_HANDLE
        .get()
        .expect("Service handle not initialized")
        .clone()
}

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type ServiceController = super::ServiceControllerRust;

        // Signals for conversation events
        #[qsignal]
        fn conversation_created(self: Pin<&mut ServiceController>, id: QString);

        #[qsignal]
        fn conversations_changed(self: Pin<&mut ServiceController>);

        #[qsignal]
        fn conversation_deleted(self: Pin<&mut ServiceController>, id: QString);

        // Signals for message events
        #[qsignal]
        fn messages_loaded(self: Pin<&mut ServiceController>, conversation_id: QString);

        #[qsignal]
        fn messages_changed(self: Pin<&mut ServiceController>, conversation_id: QString);

        // Signals for streaming
        #[qsignal]
        fn streaming_started(self: Pin<&mut ServiceController>, conversation_id: QString);

        #[qsignal]
        fn streaming_chunk(
            self: Pin<&mut ServiceController>,
            conversation_id: QString,
            delta: QString,
            accumulated: QString,
        );

        #[qsignal]
        fn streaming_complete(self: Pin<&mut ServiceController>, conversation_id: QString);

        // Error signal
        #[qsignal]
        fn error_occurred(self: Pin<&mut ServiceController>, command: QString, error: QString);

        // Actions (invokable from QML)
        #[qinvokable]
        fn new_conversation(self: &ServiceController);

        #[qinvokable]
        fn send_message(self: &ServiceController, conversation_id: &QString, text: &QString);

        #[qinvokable]
        fn load_messages(self: &ServiceController, conversation_id: &QString);

        #[qinvokable]
        fn delete_conversation(self: &ServiceController, conversation_id: &QString);

        #[qinvokable]
        fn refresh_conversations(self: &ServiceController);

        // Data access (synchronous, reads from cache)
        #[qinvokable]
        fn get_conversation_count(self: &ServiceController) -> i32;
    }

    impl cxx_qt::Threading for ServiceController {}
    impl cxx_qt::Initialize for ServiceController {}
}

use cxx_qt_lib::QString;

/// Rust implementation of ServiceController
#[derive(Default)]
pub struct ServiceControllerRust {
    // No state needed - uses global caches from qt module
}

impl cxx_qt::Initialize for ffi::ServiceController {
    fn initialize(self: Pin<&mut Self>) {
        // Take the response receiver (can only be done once)
        let response_rx = crate::qt::take_response_receiver();
        let qt_thread = self.qt_thread();

        // Spawn task to forward responses to Qt thread
        crate::get_runtime().spawn(async move {
            let mut rx = response_rx;
            while let Some(response) = rx.recv().await {
                let _ = qt_thread.queue(move |mut controller| {
                    controller.as_mut().handle_response(response);
                });
            }
        });

        // Initial load of conversations
        self.refresh_conversations();
    }
}

impl ffi::ServiceController {
    /// Handle a response from the service
    fn handle_response(mut self: Pin<&mut Self>, response: Response) {
        match response {
            Response::ConversationCreated { conversation } => {
                // Add to cache
                crate::qt::conversations_cache()
                    .lock()
                    .unwrap()
                    .push(conversation.clone());

                // Emit signals
                self.as_mut()
                    .conversation_created(QString::from(&conversation.id.to_string()));
                self.as_mut().conversations_changed();
            }

            Response::ConversationsRefreshed { conversations } => {
                // Update cache
                *crate::qt::conversations_cache().lock().unwrap() = conversations;

                // Emit signal
                self.as_mut().conversations_changed();
            }

            Response::ConversationDeleted { id } => {
                // Remove from cache
                crate::qt::conversations_cache()
                    .lock()
                    .unwrap()
                    .retain(|c| c.id != id);

                // Emit signals
                self.as_mut()
                    .conversation_deleted(QString::from(&id.to_string()));
                self.as_mut().conversations_changed();
            }

            Response::MessagesLoaded { conv_id, messages } => {
                // Update cache
                crate::qt::messages_cache()
                    .lock()
                    .unwrap()
                    .insert(conv_id.clone(), messages);

                // Emit signal
                self.as_mut()
                    .messages_loaded(QString::from(&conv_id.to_string()));
            }

            Response::MessageSent { conv_id, message } => {
                // Add to cache
                if let Some(msgs) = crate::qt::messages_cache()
                    .lock()
                    .unwrap()
                    .get_mut(&conv_id)
                {
                    msgs.push(message);
                }

                // Emit signal
                self.as_mut()
                    .messages_changed(QString::from(&conv_id.to_string()));
            }

            Response::StreamingStarted { conv_id } => {
                self.as_mut()
                    .streaming_started(QString::from(&conv_id.to_string()));
            }

            Response::StreamingChunk {
                conv_id,
                delta,
                accumulated,
            } => {
                self.as_mut().streaming_chunk(
                    QString::from(&conv_id.to_string()),
                    QString::from(&delta),
                    QString::from(&accumulated),
                );
            }

            Response::StreamingComplete { conv_id, message } => {
                // Add final message to cache
                if let Some(msgs) = crate::qt::messages_cache()
                    .lock()
                    .unwrap()
                    .get_mut(&conv_id)
                {
                    msgs.push(message);
                }

                // Emit signals
                self.as_mut()
                    .streaming_complete(QString::from(&conv_id.to_string()));
                self.as_mut()
                    .messages_changed(QString::from(&conv_id.to_string()));
            }

            Response::ConversationLoaded { conversation: _ } => {
                // Currently not used, but could be used for loading individual conversations
            }

            Response::Error { command, error } => {
                self.as_mut()
                    .error_occurred(QString::from(&command), QString::from(&error));
            }
        }
    }

    /// Create a new conversation
    fn new_conversation(&self) {
        let handle = get_service_handle();
        let _ = handle.send(Command::NewConversation);
    }

    /// Send a message in a conversation
    fn send_message(&self, conversation_id: &QString, text: &QString) {
        let handle = get_service_handle();
        let _ = handle.send(Command::SendMessage {
            conv_id: ConversationId::from(conversation_id.to_string()),
            text: text.to_string(),
        });
    }

    /// Load messages for a conversation
    fn load_messages(&self, conversation_id: &QString) {
        let handle = get_service_handle();
        let _ = handle.send(Command::LoadMessages {
            conv_id: ConversationId::from(conversation_id.to_string()),
        });
    }

    /// Delete a conversation
    fn delete_conversation(&self, conversation_id: &QString) {
        let handle = get_service_handle();
        let _ = handle.send(Command::DeleteConversation {
            id: ConversationId::from(conversation_id.to_string()),
        });
    }

    /// Refresh the list of conversations
    fn refresh_conversations(&self) {
        let handle = get_service_handle();
        let _ = handle.send(Command::RefreshConversations);
    }

    /// Get the number of conversations
    fn get_conversation_count(&self) -> i32 {
        crate::qt::conversations_cache().lock().unwrap().len() as i32
    }
}
