// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service worker that processes commands asynchronously

use futures::StreamExt;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use clareon_core::types::{ContentBlock, ConversationId, ConversationSummary, Message};
use clareon_core::{ConversationManager, StreamUpdate};

use super::{
    command::Command,
    response::{ArtifactData, ErrorCategory, ErrorInfo, MessageData, ModelInfoData, Response},
};

/// Service worker that processes commands on the tokio runtime
pub struct ServiceWorker {
    manager: ConversationManager,
    command_rx: broadcast::Receiver<Command>,
    response_tx: broadcast::Sender<Response>,
}

impl ServiceWorker {
    /// Create a new service worker
    pub fn new(
        manager: ConversationManager,
        command_rx: broadcast::Receiver<Command>,
        response_tx: broadcast::Sender<Response>,
    ) -> Self {
        Self {
            manager,
            command_rx,
            response_tx,
        }
    }

    /// Run the service worker loop
    ///
    /// This processes commands until a Shutdown command is received
    pub async fn run(mut self) {
        info!("Service worker started");

        while let Ok(cmd) = self.command_rx.recv().await {
            debug!("Processing command: {:?}", cmd);

            match cmd {
                Command::Shutdown => {
                    info!("Shutdown command received");
                    break;
                }
                cmd => self.handle_command(cmd).await,
            }
        }

        info!("Service worker stopped");
    }

    /// Handle a single command
    async fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::NewConversation => {
                self.handle_new_conversation().await;
            }
            Command::LoadConversation { id } => {
                self.handle_load_conversation(id).await;
            }
            Command::DeleteConversation { id } => {
                self.handle_delete_conversation(id).await;
            }
            Command::RefreshConversations => {
                self.handle_refresh_conversations().await;
            }
            Command::SendMessage { conv_id, text } => {
                self.handle_send_message(conv_id, text).await;
            }
            Command::SendMessageWithContent { conv_id, content } => {
                self.handle_send_message_with_content(conv_id, content)
                    .await;
            }
            Command::SendMessageWithFiles {
                conv_id,
                text,
                file_paths,
            } => {
                self.handle_send_message_with_files(conv_id, text, file_paths)
                    .await;
            }
            Command::LoadMessages { conv_id } => {
                self.handle_load_messages(conv_id).await;
            }
            Command::RetryLastMessage { conv_id } => {
                self.handle_retry_last_message(conv_id).await;
            }
            Command::Search { query } => {
                self.handle_search(query).await;
            }
            Command::NewQuickConversation { prompt } => {
                self.handle_new_quick_conversation(prompt).await;
            }
            Command::LoadArtifacts { conv_id } => {
                self.handle_load_artifacts(conv_id).await;
            }
            Command::LoadArtifact { artifact_id } => {
                self.handle_load_artifact(artifact_id).await;
            }
            Command::SaveArtifact { artifact_id, path } => {
                self.handle_save_artifact(artifact_id, path).await;
            }
            Command::Shutdown => {
                // Already handled in run() loop
            }
            Command::ActivateMainWindow => {
                self.handle_activate_main_window().await;
            }
            Command::ActivateQuickInput => {
                self.handle_activate_quick_input().await;
            }
            Command::ReloadConfig => {
                self.handle_reload_config().await;
            }
            Command::FetchAvailableModels { provider: _ } => {
                self.handle_fetch_available_models().await;
            }
        }
    }

    async fn handle_new_conversation(&self) {
        match self.manager.new_conversation().await {
            Ok(conversation) => {
                let summary = ConversationSummary {
                    id: conversation.id.clone(),
                    title: conversation.title.clone(),
                    updated_at: conversation.updated_at,
                    model: conversation.model.clone(),
                    message_count: 0, // New conversation has no messages yet
                };
                let _ = self.response_tx.send(Response::ConversationCreated {
                    conversation: summary,
                });
            }
            Err(e) => {
                error!("Failed to create conversation: {}", e);
                let _ = self.response_tx.send(Response::Error {
                    command: "NewConversation".to_string(),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_load_conversation(&self, id: clareon_core::types::ConversationId) {
        match self.manager.load_conversation(&id).await {
            Ok(conversation) => {
                let _ = self
                    .response_tx
                    .send(Response::ConversationLoaded { conversation });
            }
            Err(e) => {
                error!("Failed to load conversation {}: {}", id, e);
                let _ = self.response_tx.send(Response::Error {
                    command: format!("LoadConversation({})", id),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_delete_conversation(&self, id: clareon_core::types::ConversationId) {
        match self.manager.delete_conversation(&id).await {
            Ok(_) => {
                let _ = self.response_tx.send(Response::ConversationDeleted { id });
            }
            Err(e) => {
                error!("Failed to delete conversation {}: {}", id, e);
                let _ = self.response_tx.send(Response::Error {
                    command: format!("DeleteConversation({})", id),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_refresh_conversations(&self) {
        match self.manager.refresh_conversations().await {
            Ok(conversations) => {
                let _ = self
                    .response_tx
                    .send(Response::ConversationsRefreshed { conversations });
            }
            Err(e) => {
                error!("Failed to refresh conversations: {}", e);
                let _ = self.response_tx.send(Response::Error {
                    command: "RefreshConversations".to_string(),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_load_messages(&self, conv_id: clareon_core::types::ConversationId) {
        match self.manager.get_messages(&conv_id).await {
            Ok(messages) => {
                let messages: Vec<MessageData> =
                    messages.into_iter().map(message_to_data).collect();
                let _ = self
                    .response_tx
                    .send(Response::MessagesLoaded { conv_id, messages });
            }
            Err(e) => {
                error!(
                    "Failed to load messages for conversation {}: {}",
                    conv_id, e
                );
                let _ = self.response_tx.send(Response::Error {
                    command: format!("LoadMessages({})", conv_id),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_search(&self, query: String) {
        match self.manager.search(&query).await {
            Ok(results) => {
                debug!("Search for '{}' returned {} results", query, results.len());
                let _ = self.response_tx.send(Response::SearchResults { results });
            }
            Err(e) => {
                error!("Failed to search for '{}': {}", query, e);
                let _ = self.response_tx.send(Response::Error {
                    command: format!("Search({})", query),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_new_quick_conversation(&self, prompt: String) {
        // First create a new conversation
        let conversation = match self.manager.new_conversation().await {
            Ok(conv) => conv,
            Err(e) => {
                error!("Failed to create quick conversation: {}", e);
                let _ = self.response_tx.send(Response::Error {
                    command: "NewQuickConversation".to_string(),
                    error: e.to_string(),
                });
                return;
            }
        };

        let conv_id = conversation.id.clone();

        // Notify UI that conversation was created
        let summary = ConversationSummary {
            id: conv_id.clone(),
            title: conversation.title.clone(),
            updated_at: conversation.updated_at,
            model: conversation.model.clone(),
            message_count: 0,
        };
        let _ = self.response_tx.send(Response::ConversationCreated {
            conversation: summary,
        });

        // Now send the message (reuse the existing send message logic)
        self.handle_send_message(conv_id, prompt).await;
    }

    async fn handle_send_message(&self, conv_id: ConversationId, text: String) {
        // First store the user message to the conversation
        let user_msg_id = match self
            .manager
            .append_user_message(conv_id.clone(), &text)
            .await
        {
            Ok(msg) => {
                let msg_id = msg.id;
                let _ = self.response_tx.send(Response::MessageSent {
                    conv_id: conv_id.clone(),
                    message: message_to_data(msg),
                });
                Some(msg_id)
            }
            Err(e) => {
                error!(
                    "Failed to append user message to conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: None,
                });
                return;
            }
        };

        // Then load the conversation
        let mut conversation = match self.manager.load_conversation(&conv_id).await {
            Ok(conv) => conv,
            Err(e) => {
                error!(
                    "Failed to load conversation {} for sending message: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: user_msg_id,
                });
                return;
            }
        };

        // Start streaming
        let _ = self.response_tx.send(Response::StreamingStarted {
            conv_id: conv_id.clone(),
        });

        // Send message with streaming
        match self.manager.send_message_stream(&mut conversation).await {
            Ok(mut stream) => {
                let mut accumulated = String::new();

                // Process stream events
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(update) => {
                            if let Some(text_delta) = extract_text_delta(&update) {
                                accumulated.push_str(&text_delta);

                                let _ = self.response_tx.send(Response::StreamingChunk {
                                    conv_id: conv_id.clone(),
                                    delta: text_delta,
                                    accumulated: accumulated.clone(),
                                });
                            }
                        }
                        Err(e) => {
                            error!("Streaming error for conversation {}: {}", conv_id, e);
                            let error_info = error_to_info(&e);
                            let _ = self.response_tx.send(Response::StreamingError {
                                conv_id,
                                error_info,
                                partial_text: accumulated,
                            });
                            return;
                        }
                    }
                }

                // Streaming complete - reload messages to get the final message
                match self.manager.get_messages(&conv_id).await {
                    Ok(messages) => {
                        if let Some(last_message) = messages.last() {
                            let message_data = message_to_data(last_message.clone());
                            let _ = self.response_tx.send(Response::StreamingComplete {
                                conv_id,
                                message: message_data,
                            });
                        } else {
                            warn!("No messages found after streaming completed");
                        }
                    }
                    Err(e) => {
                        error!("Failed to reload messages after streaming: {}", e);
                    }
                }
                self.handle_refresh_conversations().await;
            }
            Err(e) => {
                error!(
                    "Failed to start streaming for conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: user_msg_id,
                });
            }
        }
    }

    async fn handle_send_message_with_content(
        &self,
        conv_id: ConversationId,
        content: Vec<ContentBlock>,
    ) {
        // First store the user message with content blocks to the conversation
        let user_msg_id = match self
            .manager
            .append_user_message_with_content(conv_id.clone(), content)
            .await
        {
            Ok(msg) => {
                let msg_id = msg.id;
                let _ = self.response_tx.send(Response::MessageSent {
                    conv_id: conv_id.clone(),
                    message: message_to_data(msg),
                });
                Some(msg_id)
            }
            Err(e) => {
                error!(
                    "Failed to append user message with content to conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: None,
                });
                return;
            }
        };

        // Then load the conversation
        let mut conversation = match self.manager.load_conversation(&conv_id).await {
            Ok(conv) => conv,
            Err(e) => {
                error!(
                    "Failed to load conversation {} for sending message: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: user_msg_id,
                });
                return;
            }
        };

        // Start streaming
        let _ = self.response_tx.send(Response::StreamingStarted {
            conv_id: conv_id.clone(),
        });

        // Send message with streaming
        match self.manager.send_message_stream(&mut conversation).await {
            Ok(mut stream) => {
                let mut accumulated = String::new();

                // Process stream events
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(update) => {
                            if let Some(text_delta) = extract_text_delta(&update) {
                                accumulated.push_str(&text_delta);

                                let _ = self.response_tx.send(Response::StreamingChunk {
                                    conv_id: conv_id.clone(),
                                    delta: text_delta,
                                    accumulated: accumulated.clone(),
                                });
                            }
                        }
                        Err(e) => {
                            error!("Streaming error for conversation {}: {}", conv_id, e);
                            let error_info = error_to_info(&e);
                            let _ = self.response_tx.send(Response::StreamingError {
                                conv_id,
                                error_info,
                                partial_text: accumulated,
                            });
                            return;
                        }
                    }
                }

                // Streaming complete - reload messages to get the final message
                match self.manager.get_messages(&conv_id).await {
                    Ok(messages) => {
                        if let Some(last_message) = messages.last() {
                            let message_data = message_to_data(last_message.clone());
                            let _ = self.response_tx.send(Response::StreamingComplete {
                                conv_id,
                                message: message_data,
                            });
                        } else {
                            warn!("No messages found after streaming completed");
                        }
                    }
                    Err(e) => {
                        error!("Failed to reload messages after streaming: {}", e);
                    }
                }
                self.handle_refresh_conversations().await;
            }
            Err(e) => {
                error!(
                    "Failed to start streaming for conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: user_msg_id,
                });
            }
        }
    }

    async fn handle_send_message_with_files(
        &self,
        conv_id: ConversationId,
        text: String,
        file_paths: Vec<String>,
    ) {
        use std::fs;
        use std::path::Path;

        // First, store the user message
        let user_msg_id = match self
            .manager
            .append_user_message(conv_id.clone(), &text)
            .await
        {
            Ok(msg) => {
                let msg_id = msg.id;
                let _ = self.response_tx.send(Response::MessageSent {
                    conv_id: conv_id.clone(),
                    message: message_to_data(msg),
                });
                msg_id
            }
            Err(e) => {
                error!(
                    "Failed to append user message to conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: None,
                });
                return;
            }
        };

        // Store each file in the database
        for path_str in &file_paths {
            let path = Path::new(path_str);

            // Read file content
            let file_content = match fs::read(path) {
                Ok(content) => content,
                Err(e) => {
                    error!("Failed to read file {}: {}", path_str, e);
                    continue;
                }
            };

            // Get filename
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => {
                    error!("Invalid filename: {}", path_str);
                    continue;
                }
            };

            // Determine MIME type from extension
            let mime_type = match path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase())
                .as_deref()
            {
                Some("txt") => "text/plain",
                Some("json") => "application/json",
                Some("xml") => "application/xml",
                Some("html") | Some("htm") => "text/html",
                Some("css") => "text/css",
                Some("js") => "text/javascript",
                Some("pdf") => "application/pdf",
                Some("zip") => "application/zip",
                Some("tar") => "application/x-tar",
                Some("gz") => "application/gzip",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("png") => "image/png",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                Some("svg") => "image/svg+xml",
                Some("mp3") => "audio/mpeg",
                Some("mp4") => "video/mp4",
                Some("csv") => "text/csv",
                Some("md") => "text/markdown",
                Some("rs") => "text/x-rust",
                Some("py") => "text/x-python",
                Some("c") => "text/x-c",
                Some("cpp") | Some("cc") | Some("cxx") => "text/x-c++",
                Some("h") | Some("hpp") => "text/x-c-header",
                Some("java") => "text/x-java",
                Some("sh") => "text/x-shellscript",
                _ => "application/octet-stream",
            };

            // Store file in database
            if let Err(e) = self
                .manager
                .storage()
                .add_user_file(&conv_id, user_msg_id, &filename, mime_type, &file_content)
                .await
            {
                error!("Failed to store file {} in database: {}", filename, e);
            } else {
                info!("Stored file {} in database", filename);
            }
        }

        // Then load the conversation and continue with streaming
        let mut conversation = match self.manager.load_conversation(&conv_id).await {
            Ok(conv) => conv,
            Err(e) => {
                error!(
                    "Failed to load conversation {} for sending message: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: Some(user_msg_id),
                });
                return;
            }
        };

        // Start streaming
        let _ = self.response_tx.send(Response::StreamingStarted {
            conv_id: conv_id.clone(),
        });

        // Send message with streaming
        match self.manager.send_message_stream(&mut conversation).await {
            Ok(mut stream) => {
                let mut accumulated = String::new();

                // Process stream events
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(update) => {
                            if let Some(text_delta) = extract_text_delta(&update) {
                                accumulated.push_str(&text_delta);

                                let _ = self.response_tx.send(Response::StreamingChunk {
                                    conv_id: conv_id.clone(),
                                    delta: text_delta,
                                    accumulated: accumulated.clone(),
                                });
                            }
                        }
                        Err(e) => {
                            error!("Streaming error for conversation {}: {}", conv_id, e);
                            let error_info = error_to_info(&e);
                            let _ = self.response_tx.send(Response::StreamingError {
                                conv_id,
                                error_info,
                                partial_text: accumulated,
                            });
                            return;
                        }
                    }
                }

                // Streaming complete - reload messages to get the final message
                match self.manager.get_messages(&conv_id).await {
                    Ok(messages) => {
                        if let Some(last_message) = messages.last() {
                            let message_data = message_to_data(last_message.clone());
                            let _ = self.response_tx.send(Response::StreamingComplete {
                                conv_id,
                                message: message_data,
                            });
                        } else {
                            warn!("No messages found after streaming completed");
                        }
                    }
                    Err(e) => {
                        error!("Failed to reload messages after streaming: {}", e);
                    }
                }
                self.handle_refresh_conversations().await;
            }
            Err(e) => {
                error!(
                    "Failed to start streaming for conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: Some(user_msg_id),
                });
            }
        }
    }

    async fn handle_retry_last_message(&self, conv_id: ConversationId) {
        // Load the conversation to get the last user message
        let mut conversation = match self.manager.load_conversation(&conv_id).await {
            Ok(conv) => conv,
            Err(e) => {
                error!("Failed to load conversation for retry: {}", e);
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: None,
                });
                return;
            }
        };

        // Start streaming (the conversation already has the user message)
        let _ = self.response_tx.send(Response::StreamingStarted {
            conv_id: conv_id.clone(),
        });

        // Send message with streaming
        match self.manager.send_message_stream(&mut conversation).await {
            Ok(mut stream) => {
                let mut accumulated = String::new();

                // Process stream events
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(update) => {
                            if let Some(text_delta) = extract_text_delta(&update) {
                                accumulated.push_str(&text_delta);

                                let _ = self.response_tx.send(Response::StreamingChunk {
                                    conv_id: conv_id.clone(),
                                    delta: text_delta,
                                    accumulated: accumulated.clone(),
                                });
                            }
                        }
                        Err(e) => {
                            error!(
                                "Streaming error during retry for conversation {}: {}",
                                conv_id, e
                            );
                            let error_info = error_to_info(&e);
                            let _ = self.response_tx.send(Response::StreamingError {
                                conv_id,
                                error_info,
                                partial_text: accumulated,
                            });
                            return;
                        }
                    }
                }

                // Streaming complete - reload messages to get the final message
                match self.manager.get_messages(&conv_id).await {
                    Ok(messages) => {
                        if let Some(last_message) = messages.last() {
                            let message_data = message_to_data(last_message.clone());
                            let _ = self.response_tx.send(Response::StreamingComplete {
                                conv_id,
                                message: message_data,
                            });
                        } else {
                            warn!("No messages found after retry streaming completed");
                        }
                    }
                    Err(e) => {
                        error!("Failed to reload messages after retry streaming: {}", e);
                    }
                }
                self.handle_refresh_conversations().await;
            }
            Err(e) => {
                error!(
                    "Failed to start streaming for retry in conversation {}: {}",
                    conv_id, e
                );
                let error_info = error_to_info(&e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info,
                    user_message_id: None,
                });
            }
        }
    }

    async fn handle_load_artifacts(&self, conv_id: ConversationId) {
        let storage = self.manager.storage();
        match storage.get_artifacts(&conv_id).await {
            Ok(artifacts) => {
                let artifacts: Vec<ArtifactData> =
                    artifacts.into_iter().map(artifact_to_data).collect();
                let _ = self
                    .response_tx
                    .send(Response::ArtifactsLoaded { conv_id, artifacts });
            }
            Err(e) => {
                error!(
                    "Failed to load artifacts for conversation {}: {}",
                    conv_id, e
                );
                let _ = self.response_tx.send(Response::Error {
                    command: format!("LoadArtifacts({})", conv_id),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_load_artifact(&self, artifact_id: i64) {
        let storage = self.manager.storage();
        match storage.get_artifact_by_id(artifact_id).await {
            Ok(artifact) => {
                info!(
                    "Loaded artifact {} ({} bytes)",
                    artifact_id,
                    artifact.content.len()
                );
                let _ = self.response_tx.send(Response::ArtifactLoaded {
                    artifact_id,
                    filename: artifact.filename,
                    mime_type: artifact
                        .mime_type
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    content: artifact.content,
                });
            }
            Err(e) => {
                error!("Failed to load artifact {}: {}", artifact_id, e);
                let _ = self.response_tx.send(Response::Error {
                    command: format!("LoadArtifact({})", artifact_id),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_activate_main_window(&self) {
        let _ = self.response_tx.send(Response::ActivateMainWindow);
    }

    async fn handle_activate_quick_input(&self) {
        let _ = self.response_tx.send(Response::ActivateQuickInput);
    }

    async fn handle_save_artifact(&self, artifact_id: i64, path: String) {
        let storage = self.manager.storage();

        match storage.get_artifact_by_id(artifact_id).await {
            Ok(artifact) => {
                // Write the artifact content to the specified path
                match tokio::fs::write(&path, &artifact.content).await {
                    Ok(_) => {
                        info!("Artifact {} saved to {}", artifact_id, path);
                        let _ = self
                            .response_tx
                            .send(Response::ArtifactSaved { artifact_id, path });
                    }
                    Err(e) => {
                        error!(
                            "Failed to write artifact {} to {}: {}",
                            artifact_id, path, e
                        );
                        let _ = self.response_tx.send(Response::Error {
                            command: format!("SaveArtifact({}, {})", artifact_id, path),
                            error: format!("Failed to write file: {}", e),
                        });
                    }
                }
            }
            Err(e) => {
                error!("Failed to get artifact {}: {}", artifact_id, e);
                let _ = self.response_tx.send(Response::Error {
                    command: format!("SaveArtifact({}, {})", artifact_id, path),
                    error: format!("Failed to get artifact: {}", e),
                });
            }
        }
    }

    async fn handle_fetch_available_models(&self) {
        match self.manager.available_models().await {
            Ok(models) => {
                let model_data: Vec<ModelInfoData> = models
                    .into_iter()
                    .map(|m| ModelInfoData {
                        id: m.id,
                        name: m.name,
                        context_window: m.context_window,
                        max_output_tokens: m.max_output_tokens,
                        description: m.description.unwrap_or_default(),
                        owner: m.owner.unwrap_or_default(),
                        pricing_prompt: m
                            .pricing
                            .as_ref()
                            .and_then(|p| p.prompt.clone())
                            .unwrap_or_default(),
                        pricing_completion: m
                            .pricing
                            .as_ref()
                            .and_then(|p| p.completion.clone())
                            .unwrap_or_default(),
                        input_modalities: m
                            .modalities
                            .as_ref()
                            .map(|mod_| mod_.input.join(","))
                            .unwrap_or_default(),
                        output_modalities: m
                            .modalities
                            .map(|mod_| mod_.output.join(","))
                            .unwrap_or_default(),
                    })
                    .collect();
                let _ = self
                    .response_tx
                    .send(Response::ModelsLoaded { models: model_data });
            }
            Err(e) => {
                error!("Failed to fetch available models: {}", e);
                let _ = self.response_tx.send(Response::ModelsLoadFailed {
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_reload_config(&mut self) {
        info!("Reloading configuration and recreating backend");

        let config = clareon_core::ConfigManager::get().config();

        // Recreate backends from new config
        let backend = match clareon_core::backend::create_backend_from_config(&config).await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to create backend after config reload: {}", e);
                let _ = self.response_tx.send(Response::Error {
                    command: "ReloadConfig".to_string(),
                    error: format!("Failed to create backend: {}", e),
                });
                return;
            }
        };
        let title_backend = match clareon_core::backend::create_backend_from_config(&config).await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to create title backend after config reload: {}", e);
                let _ = self.response_tx.send(Response::Error {
                    command: "ReloadConfig".to_string(),
                    error: format!("Failed to create title backend: {}", e),
                });
                return;
            }
        };

        // Rebuild the ConversationManager, preserving storage and tool executor
        let storage = self.manager.storage();
        let tool_executor = self.manager.tool_executor();
        let mut new_manager = ConversationManager::new(storage, backend, title_backend, config);
        if let Some(executor) = tool_executor {
            new_manager = new_manager.with_tools(executor);
        }

        self.manager = new_manager;
        info!("Configuration reloaded successfully, backend recreated");
    }
}

/// Convert a Message to MessageData for Qt consumption
fn message_to_data(message: Message) -> MessageData {
    MessageData {
        id: message.id,
        role: match message.role {
            clareon_core::types::Role::User => "user".to_string(),
            clareon_core::types::Role::Assistant => "assistant".to_string(),
        },
        text: message.text().unwrap_or("").to_string(),
        created_at: message.created_at,
        input_tokens: message.input_tokens,
        output_tokens: message.output_tokens,
    }
}

/// Convert an Artifact to ArtifactData for Qt consumption
fn artifact_to_data(artifact: clareon_core::types::Artifact) -> ArtifactData {
    ArtifactData {
        id: artifact.id,
        message_id: artifact.message_id,
        filename: artifact.filename,
        mime_type: artifact
            .mime_type
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        size_bytes: artifact.size_bytes,
        content_hash: artifact.content_hash,
        created_at: artifact.created_at,
        updated_at: artifact.updated_at,
    }
}

/// Extract text delta from a stream update
fn extract_text_delta(update: &StreamUpdate) -> Option<String> {
    use clareon_core::backend::StreamEvent;

    match &update.event {
        StreamEvent::ContentBlockDelta { delta, .. } => {
            use clareon_core::backend::ContentDelta;
            match delta {
                ContentDelta::Text { text } => Some(text.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Convert a clareon_core Error to ErrorInfo for UI display
fn error_to_info(error: &clareon_core::Error) -> ErrorInfo {
    match error {
        clareon_core::Error::Backend(backend_err) => backend_error_to_info(backend_err),
        _ => ErrorInfo {
            message: "An unexpected error occurred".to_string(),
            details: error.to_string(),
            category: ErrorCategory::Unknown,
            is_retryable: false,
            retry_after_secs: None,
        },
    }
}

/// Convert a BackendError to ErrorInfo
fn backend_error_to_info(error: &clareon_core::error::BackendError) -> ErrorInfo {
    use clareon_core::error::BackendError;

    match error {
        BackendError::Http(e) => ErrorInfo {
            message: "Network error occurred".to_string(),
            details: e.to_string(),
            category: ErrorCategory::Network,
            is_retryable: true,
            retry_after_secs: None,
        },
        BackendError::RateLimited { retry_after_secs } => ErrorInfo {
            message: if let Some(secs) = retry_after_secs {
                format!("Rate limited. Please try again in {} seconds", secs)
            } else {
                "Rate limited. Please try again later".to_string()
            },
            details: error.to_string(),
            category: ErrorCategory::RateLimit,
            is_retryable: true,
            retry_after_secs: *retry_after_secs,
        },
        BackendError::ServiceUnavailable => ErrorInfo {
            message: "Service temporarily unavailable".to_string(),
            details: error.to_string(),
            category: ErrorCategory::ServerError,
            is_retryable: true,
            retry_after_secs: None,
        },
        BackendError::Timeout => ErrorInfo {
            message: "Request timed out".to_string(),
            details: error.to_string(),
            category: ErrorCategory::Network,
            is_retryable: true,
            retry_after_secs: None,
        },
        BackendError::Authentication(msg) => ErrorInfo {
            message: "Authentication failed".to_string(),
            details: msg.clone(),
            category: ErrorCategory::Authentication,
            is_retryable: false,
            retry_after_secs: None,
        },
        BackendError::ModelNotAvailable(model) => ErrorInfo {
            message: format!("Model '{}' is not available", model),
            details: error.to_string(),
            category: ErrorCategory::ClientError,
            is_retryable: false,
            retry_after_secs: None,
        },
        BackendError::ContextLengthExceeded { max_tokens } => ErrorInfo {
            message: format!("Context length exceeded (max: {} tokens)", max_tokens),
            details: "Try starting a new conversation or removing some messages".to_string(),
            category: ErrorCategory::ContextLimit,
            is_retryable: false,
            retry_after_secs: None,
        },
        BackendError::Api { status, message } if *status >= 500 => ErrorInfo {
            message: "Server error occurred".to_string(),
            details: format!("HTTP {}: {}", status, message),
            category: ErrorCategory::ServerError,
            is_retryable: true,
            retry_after_secs: None,
        },
        _ => ErrorInfo {
            message: "An error occurred".to_string(),
            details: error.to_string(),
            category: ErrorCategory::Unknown,
            is_retryable: false,
            retry_after_secs: None,
        },
    }
}
