// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service worker that processes commands asynchronously

use futures::StreamExt;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use clareon_core::types::{ConversationId, ConversationSummary, Message};
use clareon_core::{ConversationManager, StreamUpdate};

use super::{
    command::Command,
    response::{ErrorCategory, ErrorInfo, MessageData, Response},
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
    async fn handle_command(&self, cmd: Command) {
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
            Command::Shutdown => {
                // Already handled in run() loop
            }
            Command::ActivateMainWindow => {
                self.handle_activate_main_window().await;
            }
            Command::ActivateQuickInput => {
                self.handle_activate_quick_input().await;
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

    async fn handle_activate_main_window(&self) {
        let _ = self.response_tx.send(Response::ActivateMainWindow);
    }

    async fn handle_activate_quick_input(&self) {
        let _ = self.response_tx.send(Response::ActivateQuickInput);
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
