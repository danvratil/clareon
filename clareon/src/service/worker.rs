// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service worker that processes commands asynchronously

use futures::StreamExt;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use std::sync::Arc;

use clareon_core::types::{ContentBlock, ConversationId, ConversationSummary, Message};
use clareon_core::{ConversationManager, ConversationSession, McpManager, Storage, StreamUpdate};

use super::{
    command::Command,
    response::{ArtifactData, ErrorCategory, ErrorInfo, MessageData, ModelInfoData, Response},
};

/// Service worker that processes commands on the tokio runtime
pub struct ServiceWorker {
    manager: ConversationManager,
    mcp_manager: Arc<McpManager>,
    command_rx: broadcast::Receiver<Command>,
    response_tx: broadcast::Sender<Response>,
}

impl ServiceWorker {
    /// Create a new service worker
    pub fn new(
        manager: ConversationManager,
        mcp_manager: Arc<McpManager>,
        command_rx: broadcast::Receiver<Command>,
        response_tx: broadcast::Sender<Response>,
    ) -> Self {
        Self {
            manager,
            mcp_manager,
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
            Command::FetchAvailableModels { provider } => {
                self.handle_fetch_available_models(provider).await;
            }
            Command::ListMcpServers => {
                self.handle_list_mcp_servers().await;
            }
            Command::ListMcpResources { server_id } => {
                self.handle_list_mcp_resources(server_id).await;
            }
            Command::ReadMcpResource { server_id, uri } => {
                self.handle_read_mcp_resource(server_id, uri).await;
            }
            Command::ListMcpPrompts { server_id } => {
                self.handle_list_mcp_prompts(server_id).await;
            }
            Command::GetMcpPrompt {
                server_id,
                name,
                arguments_json,
            } => {
                self.handle_get_mcp_prompt(server_id, name, arguments_json)
                    .await;
            }
            Command::InjectMcpPrompt {
                conv_id,
                server_id,
                name,
                arguments_json,
            } => {
                self.handle_inject_mcp_prompt(conv_id, server_id, name, arguments_json)
                    .await;
            }
            Command::RestartMcpServers => {
                self.handle_restart_mcp_servers().await;
            }
            Command::StartMcpOAuthLogin { server_id } => {
                self.handle_start_mcp_oauth_login(server_id).await;
            }
            Command::LogoutMcpOAuth { server_id } => {
                self.handle_logout_mcp_oauth(server_id).await;
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
        match self.manager.get_or_create_session(&id).await {
            Ok(session) => {
                let conversation = session.get_conversation().await;
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
        let messages_result =
            if let Some(session) = self.manager.get_session_if_exists(&conv_id).await {
                Ok(session.get_messages().await)
            } else {
                self.manager.get_messages(&conv_id).await
            };

        match messages_result {
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
        let session = match self.manager.get_or_create_session(&conv_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get session for conversation {}: {}", conv_id, e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id: None,
                });
                return;
            }
        };

        let user_msg_id = match session.append_user_message(&text).await {
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
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id: None,
                });
                return;
            }
        };

        let response_tx = self.response_tx.clone();
        let storage = self.manager.storage();
        tokio::spawn(async move {
            Self::run_streaming_task(session, conv_id, user_msg_id, storage, response_tx).await;
        });
    }

    async fn handle_send_message_with_content(
        &self,
        conv_id: ConversationId,
        content: Vec<ContentBlock>,
    ) {
        let session = match self.manager.get_or_create_session(&conv_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get session for conversation {}: {}", conv_id, e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id: None,
                });
                return;
            }
        };

        let user_msg_id = match session.append_user_message_with_content(content).await {
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
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id: None,
                });
                return;
            }
        };

        let response_tx = self.response_tx.clone();
        let storage = self.manager.storage();
        tokio::spawn(async move {
            Self::run_streaming_task(session, conv_id, user_msg_id, storage, response_tx).await;
        });
    }

    async fn handle_send_message_with_files(
        &self,
        conv_id: ConversationId,
        text: String,
        file_paths: Vec<String>,
    ) {
        use std::fs;
        use std::path::Path;

        let session = match self.manager.get_or_create_session(&conv_id).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get session for conversation {}: {}", conv_id, e);
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id: None,
                });
                return;
            }
        };

        // First, store the user message
        let user_msg_id = match session.append_user_message(&text).await {
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
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
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

        let response_tx = self.response_tx.clone();
        let storage = self.manager.storage();
        tokio::spawn(async move {
            Self::run_streaming_task(session, conv_id, Some(user_msg_id), storage, response_tx)
                .await;
        });
    }

    async fn handle_retry_last_message(&self, conv_id: ConversationId) {
        // The last user message is already in storage/cache — just stream from current state
        let session = match self.manager.get_or_create_session(&conv_id).await {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "Failed to get session for retry on conversation {}: {}",
                    conv_id, e
                );
                let _ = self.response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id: None,
                });
                return;
            }
        };

        let response_tx = self.response_tx.clone();
        let storage = self.manager.storage();
        tokio::spawn(async move {
            Self::run_streaming_task(session, conv_id, None, storage, response_tx).await;
        });
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

    async fn handle_fetch_available_models(&self, provider: clareon_core::config::Provider) {
        // Build a temporary config targeting the requested provider so the model
        // browser works even before the user applies a provider change in settings.
        let mut config = clareon_core::ConfigManager::get().config();
        config.default_provider = provider;

        let backend = match clareon_core::backend::create_backend_from_config(&config).await {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to create backend for model fetch: {}", e);
                let _ = self.response_tx.send(Response::ModelsLoadFailed {
                    error: format!("Failed to connect to provider: {}", e),
                });
                return;
            }
        };

        match backend.available_models().await {
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

    /// Drive a streaming response for a conversation session in a spawned task.
    ///
    /// This is called by the streaming command handlers after appending the user message.
    /// It runs independently of the command loop, allowing other conversations to stream
    /// concurrently.
    async fn run_streaming_task(
        session: std::sync::Arc<ConversationSession>,
        conv_id: ConversationId,
        user_message_id: Option<i64>,
        storage: std::sync::Arc<Storage>,
        response_tx: tokio::sync::broadcast::Sender<Response>,
    ) {
        let _ = response_tx.send(Response::StreamingStarted {
            conv_id: conv_id.clone(),
        });

        match session.send_message_stream().await {
            Ok(mut stream) => {
                let mut accumulated = String::new();

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(update) => {
                            if let Some(text_delta) = extract_text_delta(&update) {
                                accumulated.push_str(&text_delta);
                                let _ = response_tx.send(Response::StreamingChunk {
                                    conv_id: conv_id.clone(),
                                    delta: text_delta,
                                    accumulated: accumulated.clone(),
                                });
                            }
                        }
                        Err(e) => {
                            error!("Streaming error for conversation {}: {}", conv_id, e);
                            let _ = response_tx.send(Response::StreamingError {
                                conv_id,
                                error_info: error_to_info(&e),
                                partial_text: accumulated,
                            });
                            return;
                        }
                    }
                }

                // Get final message from session cache - no extra DB query needed
                let messages = session.get_messages().await;
                if let Some(last_message) = messages.last() {
                    let _ = response_tx.send(Response::StreamingComplete {
                        conv_id: conv_id.clone(),
                        message: message_to_data(last_message.clone()),
                    });
                } else {
                    warn!(
                        "No messages found after streaming completed for {}",
                        conv_id
                    );
                }

                // Refresh conversation list so the UI sees the updated timestamp/title
                match storage.list_conversations().await {
                    Ok(conversations) => {
                        let _ =
                            response_tx.send(Response::ConversationsRefreshed { conversations });
                    }
                    Err(e) => {
                        error!("Failed to refresh conversations after streaming: {}", e);
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to start streaming for conversation {}: {}",
                    conv_id, e
                );
                let _ = response_tx.send(Response::SendMessageError {
                    conv_id,
                    error_info: error_to_info(&e),
                    user_message_id,
                });
            }
        }
    }

    async fn handle_reload_config(&mut self) {
        info!("Reloading configuration and recreating backend + MCP");

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

        // Rebuild tools + MCP connections from the new config
        let storage = self.manager.storage();
        let (tool_executor, mcp_manager) = match super::build_tools(&config, storage.clone()).await
        {
            Ok(pair) => pair,
            Err(e) => {
                error!("Failed to rebuild tools after config reload: {}", e);
                let _ = self.response_tx.send(Response::Error {
                    command: "ReloadConfig".to_string(),
                    error: format!("Failed to rebuild tools: {e}"),
                });
                return;
            }
        };
        self.mcp_manager = mcp_manager;

        let mut new_manager = ConversationManager::new(storage, backend, title_backend, config);
        if let Some(executor) = tool_executor {
            new_manager = new_manager.with_tools(executor);
        }

        self.manager = new_manager;
        info!("Configuration reloaded successfully, backend and MCP recreated");

        // Push updated MCP status to UI
        self.handle_list_mcp_servers().await;
    }

    async fn handle_list_mcp_servers(&self) {
        let servers = self.mcp_manager.server_statuses().await;
        let _ = self
            .response_tx
            .send(Response::McpServersStatus { servers });
    }

    async fn handle_list_mcp_resources(&self, server_id: Option<String>) {
        let resources = self.mcp_manager.list_resources(server_id.as_deref()).await;
        let _ = self
            .response_tx
            .send(Response::McpResourcesListed { resources });
    }

    async fn handle_read_mcp_resource(&self, server_id: String, uri: String) {
        match self.mcp_manager.read_resource(&server_id, &uri).await {
            Ok(text) => {
                let _ = self.response_tx.send(Response::McpResourceRead {
                    server_id,
                    uri,
                    text,
                });
            }
            Err(e) => {
                let _ = self.response_tx.send(Response::Error {
                    command: "ReadMcpResource".to_string(),
                    error: e,
                });
            }
        }
    }

    async fn handle_list_mcp_prompts(&self, server_id: Option<String>) {
        let prompts = self.mcp_manager.list_prompts(server_id.as_deref()).await;
        let _ = self
            .response_tx
            .send(Response::McpPromptsListed { prompts });
    }

    async fn handle_get_mcp_prompt(&self, server_id: String, name: String, arguments_json: String) {
        let arguments = parse_prompt_args(&arguments_json);
        match self
            .mcp_manager
            .get_prompt(&server_id, &name, arguments)
            .await
        {
            Ok(result) => {
                let _ = self
                    .response_tx
                    .send(Response::McpPromptResolved { result });
            }
            Err(e) => {
                let _ = self.response_tx.send(Response::Error {
                    command: "GetMcpPrompt".to_string(),
                    error: e,
                });
            }
        }
    }

    async fn handle_inject_mcp_prompt(
        &self,
        conv_id: ConversationId,
        server_id: String,
        name: String,
        arguments_json: String,
    ) {
        let arguments = parse_prompt_args(&arguments_json);
        let result = match self
            .mcp_manager
            .get_prompt(&server_id, &name, arguments)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = self.response_tx.send(Response::Error {
                    command: "InjectMcpPrompt".to_string(),
                    error: e,
                });
                return;
            }
        };

        // Inject as a user message containing the flattened prompt text.
        let text = if result.text.is_empty() {
            format!("(MCP prompt `{name}` produced no content)")
        } else {
            result.text
        };

        // Prefer session path so the in-memory cache stays consistent
        match self.manager.get_or_create_session(&conv_id).await {
            Ok(session) => {
                if let Err(e) = session.append_user_message(&text).await {
                    let _ = self.response_tx.send(Response::Error {
                        command: "InjectMcpPrompt".to_string(),
                        error: e.to_string(),
                    });
                    return;
                }
            }
            Err(e) => {
                let _ = self.response_tx.send(Response::Error {
                    command: "InjectMcpPrompt".to_string(),
                    error: e.to_string(),
                });
                return;
            }
        }
        self.handle_load_messages(conv_id.clone()).await;
        let _ = self
            .response_tx
            .send(Response::McpPromptInjected { conv_id });
    }

    async fn handle_restart_mcp_servers(&mut self) {
        // Full tool rebuild (same as config reload without backend swap)
        let config = clareon_core::ConfigManager::get().config();
        let storage = self.manager.storage();
        match super::build_tools(&config, storage).await {
            Ok((tool_executor, mcp_manager)) => {
                self.mcp_manager = mcp_manager;
                self.manager.set_config(config);
                self.manager.set_tool_executor(tool_executor);
                self.handle_list_mcp_servers().await;
            }
            Err(e) => {
                let _ = self.response_tx.send(Response::Error {
                    command: "RestartMcpServers".to_string(),
                    error: e.to_string(),
                });
            }
        }
    }

    async fn handle_start_mcp_oauth_login(&mut self, server_id: String) {
        info!("Handling StartMcpOAuthLogin for '{server_id}'");
        let _ = self.response_tx.send(Response::McpOAuthStatus {
            server_id: server_id.clone(),
            message: "Starting OAuth login…".into(),
        });

        // Always use the latest saved config so OAuth flags/URLs match Settings.
        let config = clareon_core::ConfigManager::get().config();
        let Some(cfg) = config.mcp.servers.get(&server_id).cloned() else {
            let _ = self.response_tx.send(Response::McpOAuthFinished {
                server_id,
                success: false,
                message:
                    "Unknown MCP server — save settings (Apply/OK) first, then try Log in again."
                        .into(),
            });
            return;
        };
        if !cfg.oauth {
            let _ = self.response_tx.send(Response::McpOAuthFinished {
                server_id,
                success: false,
                message: "OAuth is not enabled for this server. Edit the server, enable “Use browser OAuth login”, save, then Log in."
                    .into(),
            });
            return;
        }
        if cfg
            .url
            .as_ref()
            .map(|u| u.trim().is_empty())
            .unwrap_or(true)
        {
            let _ = self.response_tx.send(Response::McpOAuthFinished {
                server_id,
                success: false,
                message: "Server has no URL configured.".into(),
            });
            return;
        }

        let _ = self.response_tx.send(Response::McpOAuthStatus {
            server_id: server_id.clone(),
            message: format!(
                "Contacting authorization server at {}…",
                cfg.url.as_deref().unwrap_or("?")
            ),
        });

        // Discovery can take a while; bound it so the UI is not stuck forever.
        let begin = match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            clareon_core::PendingOAuthLogin::begin(&server_id, &cfg),
        )
        .await
        {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                error!("OAuth begin failed for '{server_id}': {e}");
                let _ = self.response_tx.send(Response::McpOAuthFinished {
                    server_id,
                    success: false,
                    message: format!("OAuth start failed: {e}"),
                });
                return;
            }
            Err(_) => {
                error!("OAuth begin timed out for '{server_id}'");
                let _ = self.response_tx.send(Response::McpOAuthFinished {
                    server_id,
                    success: false,
                    message: "Timed out contacting the authorization server (60s). Check the URL and network.".into(),
                });
                return;
            }
        };

        let (url, pending) = begin;
        info!("OAuth authorization URL for '{server_id}': {url}");
        let _ = self.response_tx.send(Response::McpOAuthUrl {
            server_id: server_id.clone(),
            url: url.clone(),
        });
        let _ = self.response_tx.send(Response::McpOAuthStatus {
            server_id: server_id.clone(),
            message: "Opening browser for login… complete sign-in in the browser window.".into(),
        });

        if !clareon_core::open_in_browser(&url) {
            warn!("open_in_browser failed for '{server_id}'; relying on QML Qt.openUrlExternally");
            let _ = self.response_tx.send(Response::McpOAuthStatus {
                server_id: server_id.clone(),
                message: format!(
                    "Could not spawn a browser automatically. Open this URL manually:\n{url}"
                ),
            });
        }

        match pending.complete().await {
            Ok(()) => {
                let _ = self.response_tx.send(Response::McpOAuthFinished {
                    server_id: server_id.clone(),
                    success: true,
                    message: "OAuth login successful".into(),
                });
                // Rebuild tools so the server reconnects with tokens
                self.handle_restart_mcp_servers().await;
            }
            Err(e) => {
                error!("OAuth complete failed for '{server_id}': {e}");
                let _ = self.response_tx.send(Response::McpOAuthFinished {
                    server_id,
                    success: false,
                    message: e,
                });
            }
        }
    }

    async fn handle_logout_mcp_oauth(&mut self, server_id: String) {
        match self.mcp_manager.logout_oauth(&server_id).await {
            Ok(()) => {
                let _ = self.response_tx.send(Response::McpOAuthFinished {
                    server_id: server_id.clone(),
                    success: true,
                    message: "Logged out".into(),
                });
                self.handle_restart_mcp_servers().await;
            }
            Err(e) => {
                let _ = self.response_tx.send(Response::McpOAuthFinished {
                    server_id,
                    success: false,
                    message: e,
                });
            }
        }
    }
}

fn parse_prompt_args(arguments_json: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    let trimmed = arguments_json.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return None;
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(serde_json::Value::Object(map)) => Some(map),
        _ => None,
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
