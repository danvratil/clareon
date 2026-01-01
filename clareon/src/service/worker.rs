// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Service worker that processes commands asynchronously

use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use clareon_core::types::{ConversationSummary, Message};
use clareon_core::{ConversationManager, StreamUpdate};

use super::{
    command::Command,
    response::{MessageData, Response},
};

/// Service worker that processes commands on the tokio runtime
pub struct ServiceWorker {
    manager: ConversationManager,
    command_rx: mpsc::UnboundedReceiver<Command>,
    response_tx: mpsc::UnboundedSender<Response>,
}

impl ServiceWorker {
    /// Create a new service worker
    pub fn new(
        manager: ConversationManager,
        command_rx: mpsc::UnboundedReceiver<Command>,
        response_tx: mpsc::UnboundedSender<Response>,
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

        while let Some(cmd) = self.command_rx.recv().await {
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
            Command::Shutdown => {
                // Already handled in run() loop
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

    async fn handle_send_message(
        &self,
        conv_id: clareon_core::types::ConversationId,
        text: String,
    ) {
        // First load the conversation
        let mut conversation = match self.manager.load_conversation(&conv_id).await {
            Ok(conv) => conv,
            Err(e) => {
                error!(
                    "Failed to load conversation {} for sending message: {}",
                    conv_id, e
                );
                let _ = self.response_tx.send(Response::Error {
                    command: format!("SendMessage({})", conv_id),
                    error: format!("Failed to load conversation: {}", e),
                });
                return;
            }
        };

        // Start streaming
        let _ = self.response_tx.send(Response::StreamingStarted {
            conv_id: conv_id.clone(),
        });

        // Send message with streaming
        match self
            .manager
            .send_message_stream(&mut conversation, &text)
            .await
        {
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
                            let _ = self.response_tx.send(Response::Error {
                                command: format!("SendMessage({})", conv_id),
                                error: format!("Streaming error: {}", e),
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
                let _ = self.response_tx.send(Response::Error {
                    command: format!("SendMessage({})", conv_id),
                    error: e.to_string(),
                });
            }
        }
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
