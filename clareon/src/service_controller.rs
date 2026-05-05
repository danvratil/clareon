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
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type ServiceController = super::ServiceControllerRust;

        // Navigation signals
        #[qsignal]
        fn conversation_created(self: Pin<&mut ServiceController>, id: QString);

        #[qsignal]
        fn conversation_deleted(self: Pin<&mut ServiceController>, id: QString);

        // Error signal
        #[qsignal]
        fn error_occurred(self: Pin<&mut ServiceController>, command: QString, error: QString);

        // Artifact signals
        #[qsignal]
        fn artifact_loaded(
            self: Pin<&mut ServiceController>,
            artifact_id: i64,
            filename: QString,
            mime_type: QString,
            content: QString,
        );

        #[qsignal]
        fn artifact_saved(self: Pin<&mut ServiceController>, artifact_id: i64, path: QString);

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
        fn send_message_with_files(
            self: &ServiceController,
            conversation_id: &QString,
            text: &QString,
            file_paths: &QStringList,
        );

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

        #[qinvokable]
        fn load_artifact(self: &ServiceController, artifact_id: i64);

        // System integration
        #[qinvokable]
        fn set_auto_start(self: &ServiceController, enabled: bool);

        #[qinvokable]
        fn fetch_available_models(self: &ServiceController, provider: &QString);
    }

    impl cxx_qt::Threading for ServiceController {}
    impl cxx_qt::Initialize for ServiceController {}
}

use std::ops::Deref;

use clareon_core::config::Provider;
use cxx_qt_lib::{QList, QString, QStringList};

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

        crate::get_runtime().spawn(async move {
            while let Ok(response) = response_rx.recv().await {
                let _ = qt_thread.queue(move |mut controller| {
                    controller.as_mut().handle_response(response);
                });
            }
        });
    }
}

impl ffi::ServiceController {
    /// Handle a response from the service
    fn handle_response(mut self: Pin<&mut Self>, response: Response) {
        match response {
            Response::ConversationCreated { conversation } => {
                self.as_mut()
                    .conversation_created(QString::from(&conversation.id.to_string()));
            }

            Response::ConversationDeleted { id } => {
                self.as_mut()
                    .conversation_deleted(QString::from(&id.to_string()));
            }

            Response::ArtifactLoaded {
                artifact_id,
                filename,
                mime_type,
                content,
            } => {
                let content_str = if mime_type.starts_with("text/") {
                    String::from_utf8_lossy(&content).to_string()
                } else {
                    use base64::{Engine as _, engine::general_purpose};
                    general_purpose::STANDARD.encode(&content)
                };

                self.as_mut().artifact_loaded(
                    artifact_id,
                    QString::from(&filename),
                    QString::from(&mime_type),
                    QString::from(&content_str),
                );
            }

            Response::ArtifactSaved { artifact_id, path } => {
                self.as_mut()
                    .artifact_saved(artifact_id, QString::from(&path));
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

            _ => {}
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

    /// Send a message with attached files in a conversation
    fn send_message_with_files(
        &self,
        conversation_id: &QString,
        text: &QString,
        file_paths: &QStringList,
    ) {
        use std::path::Path;

        let conv_id = ConversationId::from(conversation_id.to_string());
        let handle = get_service_handle();

        // Build message text mentioning attached files
        let mut message_text = text.to_string();

        // Process each file path and collect file information
        let mut file_info = Vec::new();
        let list: &QList<QString> = file_paths.deref();
        for i in 0..list.len() {
            // Unwrap is safe here: `i` is within bounds
            let path_str = list.get(i).unwrap().to_string();
            let path = Path::new(&path_str);

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                file_info.push((path_str.clone(), filename.to_string()));
            }
        }

        // Add file list to message text if there are files
        if !file_info.is_empty() {
            if !message_text.trim().is_empty() {
                message_text.push_str("\n\n");
            }
            message_text.push_str("Attached files:\n");
            for (_, filename) in &file_info {
                message_text.push_str(&format!("- {}\n", filename));
            }
            message_text.push_str(
                "\nThese files are available in your workspace at /mnt/user-data/uploads/",
            );
        }

        // Send command with file paths so the service can store them
        let _ = handle.send(Command::SendMessageWithFiles {
            conv_id,
            text: message_text,
            file_paths: file_info.into_iter().map(|(path, _)| path).collect(),
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

    /// Load a single artifact's content
    fn load_artifact(&self, artifact_id: i64) {
        let handle = get_service_handle();
        let _ = handle.send(Command::LoadArtifact { artifact_id });
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

    /// Fetch available models for a provider
    fn fetch_available_models(&self, provider: &QString) {
        let handle = get_service_handle();
        let provider_str = provider.to_string();
        let provider = match provider_str.as_str() {
            "openai" => Provider::OpenAi,
            "openrouter" => Provider::OpenRouter,
            "litellm" => Provider::LiteLlm,
            _ => {
                tracing::warn!("Unsupported provider for model fetching: {}", provider_str);
                return;
            }
        };
        let _ = handle.send(Command::FetchAvailableModels { provider });
    }
}
