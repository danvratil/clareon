//! Conversation manager - orchestrates chat interactions

use std::sync::Arc;

use tracing::{debug, info};

use super::title::TitleGenerator;
use crate::backend::{ChatRequest, ChatResponse, LlmBackend, StopReason};
use crate::config::Config;
use crate::error::Result;
use crate::storage::Storage;
use crate::types::{Conversation, ConversationSummary, Message, SearchResult};

/// Manages conversations, orchestrating storage, LLM backends, and title generation
pub struct ConversationManager {
    storage: Storage,
    backend: Arc<dyn LlmBackend>,
    title_generator: TitleGenerator,
    config: Config,
}

impl ConversationManager {
    /// Create a new conversation manager
    pub fn new(
        storage: Storage,
        backend: Arc<dyn LlmBackend>,
        title_backend: Arc<dyn LlmBackend>,
        config: Config,
    ) -> Self {
        let title_generator =
            TitleGenerator::new(title_backend, config.models.title_generation.clone());

        Self {
            storage,
            backend,
            title_generator,
            config,
        }
    }

    /// Create a new conversation manager using the same backend for chat and title generation
    pub fn with_single_backend(
        storage: Storage,
        backend: Arc<dyn LlmBackend>,
        config: Config,
    ) -> Self {
        Self::new(storage, backend.clone(), backend, config)
    }

    /// Start a new conversation
    pub async fn new_conversation(&self) -> Result<Conversation> {
        let mut conversation = Conversation::new(&self.config.default_model);

        // Apply custom system prompt if configured
        if !self.config.system_prompt.use_default {
            if let Some(custom) = &self.config.system_prompt.custom_prompt {
                conversation.system_prompt = Some(custom.clone());
            }
        }

        // Apply custom instructions
        conversation.custom_instructions = self.config.system_prompt.custom_instructions.clone();

        let id = self.storage.create_conversation(&conversation).await?;
        conversation.id = id;

        info!("Created new conversation: {}", id);
        Ok(conversation)
    }

    /// Load an existing conversation by ID
    pub async fn load_conversation(&self, id: i64) -> Result<Conversation> {
        debug!("Loading conversation: {}", id);
        self.storage.get_conversation(id).await
    }

    /// Get all messages for a conversation
    pub async fn get_messages(&self, conversation_id: i64) -> Result<Vec<Message>> {
        self.storage.get_messages(conversation_id).await
    }

    /// Send a user message and get the assistant's response
    ///
    /// This handles the full chat turn:
    /// 1. Store the user message
    /// 2. Send to the LLM backend
    /// 3. Store the assistant response
    /// 4. Generate title after first exchange (if needed)
    pub async fn send_message(
        &self,
        conversation: &mut Conversation,
        user_input: &str,
    ) -> Result<ChatResponse> {
        // Create and store user message
        let user_message = Message::user(conversation.id, user_input);
        let user_msg_id = self.storage.add_message(&user_message).await?;
        debug!("Stored user message: {}", user_msg_id);

        // Get conversation history
        let messages = self.storage.get_messages(conversation.id).await?;

        // Build the request
        let system_prompt = self.get_effective_system_prompt(conversation);

        let request = ChatRequest::new(messages, &conversation.model)
            .with_system_prompt(system_prompt)
            .with_max_tokens(4096);

        // Send to backend
        info!("Sending request to {} backend", self.backend.name());
        let response = self.backend.send_message(&request).await?;

        // Store assistant response
        let mut assistant_message = response.message.clone();
        assistant_message.conversation_id = conversation.id;
        let assistant_msg_id = self.storage.add_message(&assistant_message).await?;
        debug!("Stored assistant message: {}", assistant_msg_id);

        // Update conversation timestamp
        conversation.touch();
        self.storage.update_conversation(conversation).await?;

        // Generate title after first exchange if not set
        if conversation.title.is_none() {
            self.maybe_generate_title(conversation, user_input, &response)
                .await?;
        }

        Ok(response)
    }

    /// Continue a conversation after tool use
    ///
    /// This is called when the assistant's previous response included tool_use blocks
    /// and the tool results need to be sent back.
    pub async fn continue_with_tool_results(
        &self,
        conversation: &mut Conversation,
        tool_result_message: Message,
    ) -> Result<ChatResponse> {
        // Store the tool result message
        self.storage.add_message(&tool_result_message).await?;

        // Get updated conversation history
        let messages = self.storage.get_messages(conversation.id).await?;

        // Build the request
        let system_prompt = self.get_effective_system_prompt(conversation);

        let request = ChatRequest::new(messages, &conversation.model)
            .with_system_prompt(system_prompt)
            .with_max_tokens(4096);

        // Send to backend
        let response = self.backend.send_message(&request).await?;

        // Store assistant response
        let mut assistant_message = response.message.clone();
        assistant_message.conversation_id = conversation.id;
        self.storage.add_message(&assistant_message).await?;

        // Update conversation
        conversation.touch();
        self.storage.update_conversation(conversation).await?;

        Ok(response)
    }

    /// List all conversations
    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.storage.list_conversations().await
    }

    /// Search conversations
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.storage.search(query).await
    }

    /// Delete a conversation
    pub async fn delete_conversation(&self, id: i64) -> Result<()> {
        info!("Deleting conversation: {}", id);
        self.storage.delete_conversation(id).await
    }

    /// Rename a conversation
    pub async fn rename_conversation(
        &self,
        conversation: &mut Conversation,
        title: &str,
    ) -> Result<()> {
        conversation.set_title(title);
        self.storage.update_conversation(conversation).await
    }

    /// Get effective system prompt for a conversation
    fn get_effective_system_prompt(&self, conversation: &Conversation) -> String {
        // Use conversation-specific system prompt if set
        let base_prompt = conversation
            .system_prompt
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                if self.config.system_prompt.use_default {
                    Config::default_system_prompt()
                } else {
                    self.config
                        .system_prompt
                        .custom_prompt
                        .as_deref()
                        .unwrap_or("")
                }
            });

        // Apply custom instructions
        let instructions = conversation.custom_instructions.as_ref().or(self
            .config
            .system_prompt
            .custom_instructions
            .as_ref());

        if let Some(inst) = instructions {
            format!("{}\n\n{}", base_prompt, inst)
        } else {
            base_prompt.to_string()
        }
    }

    /// Generate title after first exchange if appropriate
    async fn maybe_generate_title(
        &self,
        conversation: &mut Conversation,
        user_input: &str,
        response: &ChatResponse,
    ) -> Result<()> {
        // Only generate title if this appears to be the first exchange
        // and the response was successful (end_turn, not tool_use)
        if response.stop_reason != StopReason::EndTurn {
            return Ok(());
        }

        let assistant_text = response.message.text().unwrap_or("");

        match self
            .title_generator
            .generate_title(user_input, assistant_text)
            .await
        {
            Ok(title) => {
                info!("Generated title: {}", title);
                conversation.set_title(&title);
                self.storage.update_conversation(conversation).await?;
            }
            Err(e) => {
                // Title generation failure shouldn't break the chat
                tracing::warn!("Failed to generate title: {}", e);
            }
        }

        Ok(())
    }

    /// Get the current backend name
    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    /// Get the current model
    pub fn current_model(&self) -> &str {
        &self.config.default_model
    }
}
