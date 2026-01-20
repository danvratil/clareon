// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
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

/// Try to get the global service handle (returns None if not initialized)
pub fn try_get_service_handle() -> Option<ServiceHandle> {
    SERVICE_HANDLE.get().cloned()
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

        // Search signal
        #[qsignal]
        fn search_results_ready(self: Pin<&mut ServiceController>);

        #[qsignal]
        fn main_window_requested(self: Pin<&mut ServiceController>);

        #[qsignal]
        fn quick_input_requested(self: Pin<&mut ServiceController>);

        // Actions (invokable from QML)
        #[qinvokable]
        fn new_conversation(self: &ServiceController);

        #[qinvokable]
        fn send_message(self: &ServiceController, conversation_id: &QString, text: &QString);

        #[qinvokable]
        fn load_messages(self: &ServiceController, conversation_id: &QString);

        #[qinvokable]
        fn retry_last_message(self: &ServiceController, conversation_id: &QString);

        #[qinvokable]
        fn delete_conversation(self: &ServiceController, conversation_id: &QString);

        #[qinvokable]
        fn refresh_conversations(self: &ServiceController);

        #[qinvokable]
        fn search(self: &ServiceController, query: &QString);

        #[qinvokable]
        fn new_quick_conversation(self: &ServiceController, prompt: &QString);

        // System integration
        #[qinvokable]
        fn set_auto_start(self: &ServiceController, enabled: bool);

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
        // Subscribe to broadcast events
        let handle = get_service_handle();
        let mut response_rx = handle.subscribe();
        let qt_thread = self.qt_thread();

        // Spawn task to forward responses to Qt thread
        crate::get_runtime().spawn(async move {
            while let Ok(response) = response_rx.recv().await {
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

            Response::MessagesLoaded { conv_id, .. } => {
                // MessageListModel now handles this directly
                // Emit signal for UI feedback
                self.as_mut()
                    .messages_loaded(QString::from(&conv_id.to_string()));
            }

            Response::MessageSent { conv_id, .. } => {
                // MessageListModel now handles this directly
                // Emit signal for UI feedback
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

            Response::StreamingComplete { conv_id, .. } => {
                // MessageListModel now handles this directly
                // Emit signals for UI feedback
                self.as_mut()
                    .streaming_complete(QString::from(&conv_id.to_string()));
                self.as_mut()
                    .messages_changed(QString::from(&conv_id.to_string()));
            }

            Response::ConversationLoaded { conversation: _ } => {
                // Currently not used, but could be used for loading individual conversations
            }

            Response::SendMessageError {
                conv_id,
                error_info,
                user_message_id: _,
            } => {
                // For now, emit error_occurred signal
                // Later, MessageListModel will handle this directly
                self.as_mut().error_occurred(
                    QString::from(&format!("SendMessage({})", conv_id)),
                    QString::from(&error_info.message),
                );
            }

            Response::StreamingError {
                conv_id,
                error_info,
                partial_text: _,
            } => {
                // For now, emit error_occurred signal
                // Later, MessageListModel will handle this directly
                self.as_mut().error_occurred(
                    QString::from(&format!("StreamingError({})", conv_id)),
                    QString::from(&error_info.message),
                );
            }

            Response::SearchResults { results } => {
                // Update cache
                *crate::qt::search_results_cache().lock().unwrap() = results;

                // Emit signal
                self.as_mut().search_results_ready();
            }

            Response::Error { command, error } => {
                self.as_mut()
                    .error_occurred(QString::from(&command), QString::from(&error));
            }

            Response::ActivateMainWindow => {
                self.as_mut().main_window_requested();
            }

            Response::ActivateQuickInput => {
                self.as_mut().quick_input_requested();
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

    /// Retry the last failed message in a conversation
    fn retry_last_message(&self, conversation_id: &QString) {
        let handle = get_service_handle();
        let _ = handle.send(Command::RetryLastMessage {
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

    /// Search across all conversations
    fn search(&self, query: &QString) {
        let handle = get_service_handle();
        let _ = handle.send(Command::Search {
            query: query.to_string(),
        });
    }

    /// Set auto-start on login
    fn set_auto_start(&self, enabled: bool) {
        let xdg_dirs = xdg::BaseDirectories::new();

        if enabled {
            let desktop_file = match xdg_dirs.place_config_file("autostart/cc.clareon.desktop") {
                Ok(path) => path,
                Err(e) => {
                    tracing::error!("Failed to determine autostart desktop file path: {}", e);
                    return;
                }
            };

            std::fs::write(
                desktop_file,
                r#"
[Desktop Entry]
Name=Clareon
GenericName=Clareon
Exec=clareon
Icon=clareon
StartupNotify=true
Terminal=false
Type=Application
Version=1.0
Categories=Utility;Qt
X-GNOME-Autostart-enabled=true
X-GNOME-Autostart-Delay=2
X-KDE-autostart-after=panel
X-LXQt-Need-Tray=true"#,
            )
            .unwrap_or_else(|e| {
                tracing::error!("Failed to write autostart desktop file: {}", e);
            });
        } else if let Some(desktop_file) = xdg_dirs.get_config_file("autostart/cc.clareon.desktop")
        {
            std::fs::remove_file(&desktop_file).unwrap_or_else(|e| {
                tracing::error!("Failed to remove autostart desktop file: {}", e);
            });
        }
    }
    /// Create a new conversation and immediately send a message
    /// Used for quick input flow
    fn new_quick_conversation(&self, prompt: &QString) {
        let handle = get_service_handle();
        let _ = handle.send(Command::NewQuickConversation {
            prompt: prompt.to_string(),
        });
    }

    /// Get the number of conversations
    fn get_conversation_count(&self) -> i32 {
        crate::qt::conversations_cache().lock().unwrap().len() as i32
    }
}
