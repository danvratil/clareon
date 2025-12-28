// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conversation manager - orchestrates chat interactions

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{Stream, StreamExt};
use tracing::{debug, info};

use super::title::TitleGenerator;
use crate::backend::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, StopReason, StreamEvent, Usage,
};
use crate::config::Config;
use crate::error::Result;
use crate::storage::Storage;
use crate::tools::ToolExecutor;
use crate::types::{ContentBlock, Conversation, ConversationSummary, Message, Role, SearchResult};

/// Update from streaming message containing both the event and accumulated state
#[derive(Debug, Clone)]
pub struct StreamUpdate {
    /// The raw event from the backend
    pub event: StreamEvent,

    /// Accumulated content blocks so far
    pub partial_content: Vec<ContentBlock>,

    /// Stop reason (if message has ended)
    pub stop_reason: Option<StopReason>,

    /// Accumulated usage information
    pub usage: Usage,
}

/// Manages conversations, orchestrating storage, LLM backends, and title generation
pub struct ConversationManager {
    storage: Arc<Storage>,
    backend: Arc<dyn LlmBackend>,
    title_generator: Arc<TitleGenerator>,
    config: Config,
    tool_executor: Option<Arc<ToolExecutor>>,
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
            storage: Arc::new(storage),
            backend,
            title_generator: Arc::new(title_generator),
            config,
            tool_executor: None,
        }
    }

    /// Set the tool executor for this conversation manager
    pub fn with_tools(mut self, executor: Arc<ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// Create a new conversation manager using the same backend for chat and title generation
    pub fn with_single_backend(
        storage: Storage,
        backend: Arc<dyn LlmBackend>,
        config: Config,
    ) -> Self {
        Self::new(storage, backend.clone(), backend, config)
    }

    /// Get a reference to the storage
    pub fn storage(&self) -> Arc<Storage> {
        self.storage.clone()
    }

    /// Start a new conversation
    pub async fn new_conversation(&self) -> Result<Conversation> {
        let mut conversation = Conversation::new(&self.config.default_model);

        // Apply custom system prompt if configured
        if !self.config.system_prompt.use_default
            && let Some(custom) = &self.config.system_prompt.custom_prompt
        {
            conversation.system_prompt = Some(custom.clone());
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

    /// Send a user message and stream the assistant's response
    ///
    /// This handles the full chat turn with streaming:
    /// 1. Store the user message
    /// 2. Stream from the LLM backend
    /// 3. Accumulate and forward streaming events
    /// 4. Store the complete assistant response when done
    /// 5. Generate title after first exchange (if needed)
    pub async fn send_message_stream(
        &self,
        conversation: &mut Conversation,
        user_input: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate>> + Send>>> {
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

        // Start backend stream
        info!("Streaming request to {} backend", self.backend.name());
        let backend_stream = self.backend.send_message_stream(&request).await?;

        // Set up state accumulation
        let conv_id = conversation.id;
        let storage = self.storage.clone();
        let model = conversation.model.clone();
        let user_input_clone = user_input.to_string();
        let title_generator = self.title_generator.clone();
        let mut conv_for_title = conversation.clone();

        // Create stream that accumulates state and stores message at the end
        let stream = async_stream::stream! {
            let mut content_blocks: Vec<ContentBlock> = Vec::new();
            let mut stop_reason: Option<StopReason> = None;
            let mut usage = Usage::default();

            // Process backend stream events
            let mut backend_stream = Box::pin(backend_stream);
            while let Some(result) = backend_stream.next().await {
                match result {
                    Ok(event) => {
                        // Update accumulated state based on event
                        match &event {
                            StreamEvent::ContentBlockStart { index, block } => {
                                // Ensure we have enough slots
                                while content_blocks.len() <= *index {
                                    content_blocks.push(ContentBlock::Text { text: String::new() });
                                }
                                content_blocks[*index] = block.clone();
                            }
                            StreamEvent::ContentBlockDelta { index, delta } => {
                                // Ensure we have a block at this index (Bedrock doesn't send
                                // ContentBlockStart for text blocks)
                                while content_blocks.len() <= *index {
                                    content_blocks.push(ContentBlock::Text { text: String::new() });
                                }
                                if let Some(block) = content_blocks.get_mut(*index) {
                                    Self::apply_delta(block, delta);
                                }
                            }
                            StreamEvent::MessageStop { stop_reason: sr } => {
                                stop_reason = Some(*sr);
                            }
                            StreamEvent::Usage(u) => {
                                usage = *u;
                            }
                            _ => {}
                        }

                        // Forward the event with accumulated state
                        yield Ok(StreamUpdate {
                            event,
                            partial_content: content_blocks.clone(),
                            stop_reason,
                            usage,
                        });
                    }
                    Err(e) => {
                        yield Err(e.into());
                        return; // End stream on error
                    }
                }
            }

            // Stream complete - store the final message
            let message = Message::assistant(
                conv_id,
                content_blocks,
                &model,
                usage.input_tokens,
                usage.output_tokens,
            );

            if let Err(e) = storage.add_message(&message).await {
                yield Err(e);
                return;
            }

            // Update conversation timestamp
            conv_for_title.touch();
            if let Err(e) = storage.update_conversation(&conv_for_title).await {
                yield Err(e);
                return;
            }

            // Generate title if needed (don't block on this)
            if conv_for_title.title.is_none() && stop_reason == Some(StopReason::EndTurn) {
                let assistant_text = message.text().unwrap_or("");
                if let Ok(title) = title_generator.generate_title(&user_input_clone, assistant_text).await {
                    info!("Generated title: {}", title);
                    conv_for_title.set_title(&title);
                    let _ = storage.update_conversation(&conv_for_title).await;
                }
            }
        };

        Ok(Box::pin(stream))
    }

    /// Apply a content delta to a content block
    fn apply_delta(block: &mut ContentBlock, delta: &ContentDelta) {
        match (block, delta) {
            (ContentBlock::Text { text }, ContentDelta::Text { text: delta_text }) => {
                text.push_str(delta_text);
            }
            (ContentBlock::ToolUse { input, .. }, ContentDelta::ToolInput { partial_json }) => {
                // For tool input, we need to accumulate the JSON string
                // It will be parsed later when the block is complete
                if let serde_json::Value::String(current) = input {
                    current.push_str(partial_json);
                } else {
                    // Initialize as string if not already
                    *input = serde_json::Value::String(partial_json.clone());
                }
            }
            _ => {
                // Mismatched types - shouldn't happen
                debug!("Mismatched content block and delta types");
            }
        }
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

    /// Send a message with automatic tool execution loop
    ///
    /// This is similar to send_message but handles tool use automatically:
    /// 1. Send user message
    /// 2. If response contains tool uses, execute them
    /// 3. Send tool results back and repeat
    /// 4. Continue until response has stop_reason != ToolUse
    pub async fn send_message_with_tools(
        &self,
        conversation: &mut Conversation,
        user_input: &str,
    ) -> Result<ChatResponse> {
        const MAX_TOOL_ITERATIONS: usize = 5;

        // Create and store user message
        let user_message = Message::user(conversation.id, user_input);
        self.storage.add_message(&user_message).await?;

        let mut iteration = 0;
        loop {
            iteration += 1;

            if iteration > MAX_TOOL_ITERATIONS {
                return Err(crate::error::Error::Tool(
                    crate::tools::ToolError::ExecutionFailed(
                        "Too many tool iterations".to_string(),
                    ),
                ));
            }

            // Get conversation history
            let messages = self.storage.get_messages(conversation.id).await?;

            // Build request with tool definitions if executor is available
            let system_prompt = self.get_effective_system_prompt(conversation);
            let mut request = ChatRequest::new(messages, &conversation.model)
                .with_system_prompt(system_prompt)
                .with_max_tokens(4096);

            // Add tool definitions if tools are enabled
            if let Some(executor) = &self.tool_executor
                && self.config.tools.enabled
            {
                request.tools = executor.registry.tool_definitions();
            }

            // Send to backend
            info!(
                "Sending request to {} backend (iteration {})",
                self.backend.name(),
                iteration
            );
            let response = self.backend.send_message(&request).await?;

            // Store assistant response
            let mut assistant_message = response.message.clone();
            assistant_message.conversation_id = conversation.id;
            let assistant_msg_id = self.storage.add_message(&assistant_message).await?;

            // Check stop reason
            match response.stop_reason {
                StopReason::EndTurn | StopReason::MaxTokens | StopReason::StopSequence => {
                    // Natural end - update conversation and return
                    conversation.touch();
                    self.storage.update_conversation(conversation).await?;

                    if conversation.title.is_none() && iteration == 1 {
                        self.maybe_generate_title(conversation, user_input, &response)
                            .await?;
                    }

                    return Ok(response);
                }
                StopReason::ToolUse => {
                    // Execute tools and continue
                    if let Some(executor) = &self.tool_executor {
                        info!("Tool use requested, executing tools");
                        let tool_results = self
                            .execute_tools(
                                &response.message,
                                executor,
                                conversation.id,
                                assistant_msg_id,
                            )
                            .await?;

                        // Store tool results as user message
                        let tool_result_message = Message {
                            id: 0,
                            conversation_id: conversation.id,
                            created_at: chrono::Utc::now().timestamp(),
                            role: Role::User,
                            text_content: None,
                            content: tool_results,
                            input_tokens: None,
                            output_tokens: None,
                            model: None,
                        };
                        self.storage.add_message(&tool_result_message).await?;

                        // Continue loop to send tool results back
                    } else {
                        return Err(crate::error::Error::Tool(
                            crate::tools::ToolError::ExecutionFailed(
                                "Model requested tools but executor not configured".to_string(),
                            ),
                        ));
                    }
                }
            }
        }
    }

    /// Execute all tool uses in a message
    async fn execute_tools(
        &self,
        message: &Message,
        executor: &ToolExecutor,
        conversation_id: i64,
        message_id: i64,
    ) -> Result<Vec<ContentBlock>> {
        let mut tool_uses = Vec::new();

        for block in &message.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                tool_uses.push((id.as_str(), name.as_str(), input));
            }
        }

        if tool_uses.is_empty() {
            return Ok(Vec::new());
        }

        info!("Executing {} tools", tool_uses.len());

        // Execute tools with artifact synchronization
        let timeout = Duration::from_secs(self.config.tools.default_timeout);
        tokio::select! {
            results = executor.execute_multiple(tool_uses, conversation_id, message_id) => {
                results.map_err(crate::error::Error::Tool)
            },
            _ = tokio::time::sleep(timeout) => {
                Err(crate::error::Error::Tool(
                    crate::tools::ToolError::Timeout(timeout),
                ))
            }
        }
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
        let base_prompt = conversation.system_prompt.as_deref().unwrap_or_else(|| {
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
