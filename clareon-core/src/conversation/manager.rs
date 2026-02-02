// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conversation manager - orchestrates chat interactions

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{Stream, StreamExt};
use tracing::{debug, info, warn};

use super::title::TitleGenerator;
use crate::backend::{
    ChatRequest, ChatResponse, ContentDelta, LlmBackend, StopReason, StreamEvent, Usage,
};
use crate::config::Config;
use crate::error::Result;
use crate::storage::Storage;
use crate::tools::ToolExecutor;
use crate::types::{
    ContentBlock, Conversation, ConversationId, ConversationSummary, Message, Role, SearchResult,
};

/// Status of tool execution during streaming
#[derive(Debug, Clone)]
pub enum ToolExecutionStatus {
    /// Tools are currently being executed
    ExecutingTools {
        /// Number of tools being executed
        count: usize,
    },

    /// Tools completed successfully
    ToolsComplete {
        /// Brief summaries of results (first 100 chars of each)
        results: Vec<String>,
    },

    /// Tool execution failed
    ToolError {
        /// Error message
        error: String,
    },
}

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

    /// Tool execution status (if tools are being used)
    pub tool_execution_status: Option<ToolExecutionStatus>,

    /// Current tool execution iteration (starts at 1)
    pub iteration: usize,
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
        storage: Arc<Storage>,
        backend: Arc<dyn LlmBackend>,
        title_backend: Arc<dyn LlmBackend>,
        config: Config,
    ) -> Self {
        let title_generator =
            TitleGenerator::new(title_backend, config.models.title_generation.clone());

        Self {
            storage,
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
        storage: Arc<Storage>,
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
        let model = &self.backend.default_model().id;
        let mut conversation = Conversation::new(model);

        // Apply custom system prompt if configured
        if !self.config.system_prompt.use_default
            && let Some(custom) = &self.config.system_prompt.custom_prompt
        {
            conversation.system_prompt = Some(custom.clone());
        }

        // Apply custom instructions
        conversation.custom_instructions = self.config.system_prompt.custom_instructions.clone();

        let id = self.storage.create_conversation(&conversation).await?;
        conversation.id = id.clone();

        info!("Created new conversation: {}", id);
        Ok(conversation)
    }

    /// Load an existing conversation by ID
    pub async fn load_conversation(&self, id: &ConversationId) -> Result<Conversation> {
        debug!("Loading conversation: {}", id);
        self.storage.get_conversation(id).await
    }

    /// Get all messages for a conversation
    pub async fn get_messages(&self, conversation_id: &ConversationId) -> Result<Vec<Message>> {
        let messages = self.storage.get_messages(conversation_id).await?;
        Ok(messages)
    }

    /// Check if tools should be enabled for requests
    fn should_use_tools(&self) -> bool {
        self.tool_executor.is_some() && self.config.tools.enabled
    }

    /// Send a user message and get the assistant's response
    ///
    /// This handles the full chat turn:
    /// 1. Store the user message
    /// 2. Send to the LLM backend (with tools if enabled)
    /// 3. Execute tools automatically if requested by the model
    /// 4. Store the assistant response
    /// 5. Generate title after first exchange (if needed)
    ///
    /// Tool execution is automatic when `config.tools.enabled` is true and
    /// a tool executor is configured. The method will loop up to MAX_TOOL_ITERATIONS
    /// times to handle multi-turn tool execution.
    pub async fn send_message(
        &self,
        conversation: &mut Conversation,
        user_input: &str,
    ) -> Result<ChatResponse> {
        // Create and store user message
        let user_message = Message::user(conversation.id.clone(), user_input);
        self.storage.add_message(&user_message).await?;

        if self.should_use_tools() {
            // Use tool execution loop
            const MAX_TOOL_ITERATIONS: usize = 5;
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
                let messages = self.storage.get_messages(&conversation.id).await?;

                // Build request with tool definitions
                let system_prompt = self.get_effective_system_prompt(conversation);
                let mut request = ChatRequest::new(messages, &conversation.model)
                    .with_system_prompt(system_prompt)
                    .with_max_tokens(4096);

                // Add tool definitions
                if let Some(executor) = &self.tool_executor {
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
                assistant_message.conversation_id = conversation.id.clone();
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
                                    &conversation.id,
                                    assistant_msg_id,
                                )
                                .await?;

                            // Store tool results as user message
                            let tool_result_message = Message {
                                id: 0,
                                conversation_id: conversation.id.clone(),
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
        } else {
            // Simple single call without tools
            let messages = self.storage.get_messages(&conversation.id).await?;

            let system_prompt = self.get_effective_system_prompt(conversation);
            let request = ChatRequest::new(messages, &conversation.model)
                .with_system_prompt(system_prompt)
                .with_max_tokens(4096);

            info!("Sending request to {} backend", self.backend.name());
            let response = self.backend.send_message(&request).await?;

            let mut assistant_message = response.message.clone();
            assistant_message.conversation_id = conversation.id.clone();
            self.storage.add_message(&assistant_message).await?;

            conversation.touch();
            self.storage.update_conversation(conversation).await?;

            if conversation.title.is_none() {
                self.maybe_generate_title(conversation, user_input, &response)
                    .await?;
            }

            Ok(response)
        }
    }

    /// Appends user message to the store and returns the updated message.
    pub async fn append_user_message(
        &self,
        conv_id: ConversationId,
        user_input: &str,
    ) -> Result<Message> {
        // Create and store user message
        let mut user_message = Message::user(conv_id, user_input);
        let user_msg_id = self.storage.add_message(&user_message).await?;
        user_message.id = user_msg_id;
        debug!("Stored user message: {}", user_msg_id);
        Ok(user_message)
    }

    /// Appends user message with custom content blocks to the store and returns the updated message.
    pub async fn append_user_message_with_content(
        &self,
        conv_id: ConversationId,
        content: Vec<ContentBlock>,
    ) -> Result<Message> {
        // Create and store user message
        let mut user_message = Message::user_with_content(conv_id, content);
        let user_msg_id = self.storage.add_message(&user_message).await?;
        user_message.id = user_msg_id;
        debug!("Stored user message with custom content: {}", user_msg_id);
        Ok(user_message)
    }

    /// Send a message with streaming response and automatic tool execution
    ///
    /// This handles the full chat turn with streaming and automatic tool execution:
    /// 1. Stream from the LLM backend
    /// 2. When tool use is detected, execute tools
    /// 3. Send tool results back and continue streaming
    /// 4. Repeat until natural end (no more tool use)
    /// 5. Store messages and generate title as needed
    ///
    /// The stream yields StreamUpdate events that include:
    /// - Text deltas as they arrive
    /// - Tool use blocks with accumulated JSON inputs
    /// - Tool execution status updates
    /// - Final stop reason and usage
    pub async fn send_message_stream(
        &self,
        conversation: &mut Conversation,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate>> + Send>>> {
        const MAX_TOOL_ITERATIONS: usize = 5;

        // Clone what we need for the stream
        let storage = self.storage.clone();
        let backend = self.backend.clone();
        let conv_id = conversation.id.clone();
        let model = conversation.model.clone();
        let title_generator = self.title_generator.clone();
        let tool_executor = self.tool_executor.clone();
        let config = self.config.clone();
        let mut conv_for_title = conversation.clone();

        // Get the last user message for title generation
        let messages = self.storage.get_messages(&conversation.id).await?;
        let user_input = messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .and_then(|m| m.text())
            .map(|s| s.to_string());

        // Create the main streaming loop
        let stream = async_stream::stream! {
            let mut iteration = 0;

            loop {
                iteration += 1;

                if iteration > MAX_TOOL_ITERATIONS {
                    yield Err(crate::error::Error::Tool(
                        crate::tools::ToolError::ExecutionFailed(
                            format!("Too many tool iterations (max: {})", MAX_TOOL_ITERATIONS)
                        ),
                    ));
                    return;
                }

                // Get conversation history
                let messages = match storage.get_messages(&conv_id).await {
                    Ok(msgs) => msgs,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                // Build the request
                let system_prompt = Self::build_system_prompt(&config, &conv_for_title);
                let mut request = ChatRequest::new(messages, &model)
                    .with_system_prompt(system_prompt)
                    .with_max_tokens(4096);

                // Add tool definitions if tools are enabled
                if let Some(ref executor) = tool_executor
                    && config.tools.enabled {
                        let tools = executor.registry.tool_definitions();
                        debug!("Adding {} tools to request (iteration {})", tools.len(), iteration);
                        request.tools = tools;
                    } else {
                        debug!("Not adding tools - executor: {}, enabled: {}",
                            tool_executor.is_some(), config.tools.enabled);
                    }

                // Start backend stream
                info!(
                    "Streaming request to {} backend (iteration {})",
                    backend.name(),
                    iteration
                );
                let backend_stream = match backend.send_message_stream(&request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        yield Err(e.into());
                        return;
                    }
                };

                // Accumulate the response
                let mut content_blocks: Vec<ContentBlock> = Vec::new();
                let mut stop_reason: Option<StopReason> = None;
                let mut usage = Usage::default();
                let mut tool_input_accumulators: HashMap<usize, String> = HashMap::new();

                // Process backend stream events
                let mut backend_stream = Box::pin(backend_stream);
                while let Some(result) = backend_stream.next().await {
                    match result {
                        Ok(event) => {
                            // Update accumulated state based on event
                            match &event {
                                StreamEvent::ContentBlockStart { index, block } => {
                                    while content_blocks.len() <= *index {
                                        content_blocks.push(ContentBlock::Text { text: String::new() });
                                    }
                                    content_blocks[*index] = block.clone();

                                    if matches!(block, ContentBlock::ToolUse { .. }) {
                                        tool_input_accumulators.insert(*index, String::new());
                                    }
                                }
                                StreamEvent::ContentBlockDelta { index, delta } => {
                                    while content_blocks.len() <= *index {
                                        content_blocks.push(ContentBlock::Text { text: String::new() });
                                    }

                                    match delta {
                                        ContentDelta::Text { text: delta_text } => {
                                            if let Some(ContentBlock::Text { text }) = content_blocks.get_mut(*index) {
                                                text.push_str(delta_text);
                                            }
                                        }
                                        ContentDelta::ToolInput { partial_json } => {
                                            tool_input_accumulators
                                                .entry(*index)
                                                .or_default()
                                                .push_str(partial_json);

                                            if let Some(ContentBlock::ToolUse { input, .. }) = content_blocks.get_mut(*index) {
                                                *input = serde_json::Value::String(
                                                    tool_input_accumulators[index].clone()
                                                );
                                            }
                                        }
                                    }
                                }
                                StreamEvent::ContentBlockStop { index } => {
                                    if let Some(json_str) = tool_input_accumulators.remove(index)
                                        && let Some(ContentBlock::ToolUse { input, .. }) = content_blocks.get_mut(*index) {
                                            *input = serde_json::from_str(&json_str)
                                                .unwrap_or_else(|e| {
                                                    warn!("Failed to parse tool input JSON: {}", e);
                                                    serde_json::json!({ "error": "Invalid JSON" })
                                                });
                                        }
                                }
                                StreamEvent::MessageStop { stop_reason: sr } => {
                                    stop_reason = Some(*sr);
                                }
                                StreamEvent::Usage(u) => {
                                    usage = *u;
                                }
                            }

                            // Forward the event with accumulated state
                            yield Ok(StreamUpdate {
                                event,
                                partial_content: content_blocks.clone(),
                                stop_reason,
                                usage,
                                tool_execution_status: None,
                                iteration,
                            });
                        }
                        Err(e) => {
                            yield Err(e.into());
                            return;
                        }
                    }
                }

                // Store the assistant message
                let message = Message::assistant(
                    conv_id.clone(),
                    content_blocks.clone(),
                    &model,
                    usage.input_tokens,
                    usage.output_tokens,
                );

                let assistant_msg_id = match storage.add_message(&message).await {
                    Ok(id) => id,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                // Check stop reason and handle tool execution
                match stop_reason {
                    Some(StopReason::ToolUse) => {
                        // Count tool uses
                        let tool_count = content_blocks.iter().filter(|b| b.is_tool_use()).count();

                        if tool_count == 0 {
                            warn!("Stop reason is ToolUse but no tool use blocks found");
                            break;
                        }

                        // Check if we have a tool executor
                        let Some(ref executor) = tool_executor else {
                            yield Err(crate::error::Error::Tool(
                                crate::tools::ToolError::ExecutionFailed(
                                    "Model requested tools but executor not configured".to_string()
                                ),
                            ));
                            return;
                        };

                        // Emit tool execution start event
                        yield Ok(StreamUpdate {
                            event: StreamEvent::MessageStop { stop_reason: StopReason::ToolUse },
                            partial_content: content_blocks.clone(),
                            stop_reason: Some(StopReason::ToolUse),
                            usage,
                            tool_execution_status: Some(ToolExecutionStatus::ExecutingTools { count: tool_count }),
                            iteration,
                        });

                        // Execute tools
                        info!("Executing {} tools (iteration {})", tool_count, iteration);
                        let tool_results = match Self::execute_tools_static(
                            &message,
                            executor,
                            &conv_id,
                            assistant_msg_id,
                            &config,
                        ).await {
                            Ok(results) => results,
                            Err(e) => {
                                // Emit error event
                                yield Ok(StreamUpdate {
                                    event: StreamEvent::MessageStop { stop_reason: StopReason::ToolUse },
                                    partial_content: content_blocks.clone(),
                                    stop_reason: Some(StopReason::ToolUse),
                                    usage,
                                    tool_execution_status: Some(ToolExecutionStatus::ToolError {
                                        error: e.to_string(),
                                    }),
                                    iteration,
                                });

                                // Create tool error message to send back to model
                                let error_message = Self::create_tool_error_message(&conv_id, &e);
                                if let Err(store_err) = storage.add_message(&error_message).await {
                                    yield Err(store_err);
                                    return;
                                }

                                // Continue loop so model can react to the error
                                continue;
                            }
                        };

                        // Extract result summaries (first 100 chars of each)
                        let result_summaries: Vec<String> = tool_results
                            .iter()
                            .filter_map(|block| {
                                if let ContentBlock::ToolResult { content, .. } = block {
                                    content.first().map(|c| {
                                        let crate::types::ToolResultContent::Text { text } = c;
                                        text.chars().take(100).collect::<String>()
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect();

                        // Emit tool completion event
                        yield Ok(StreamUpdate {
                            event: StreamEvent::MessageStop { stop_reason: StopReason::ToolUse },
                            partial_content: content_blocks,
                            stop_reason: Some(StopReason::ToolUse),
                            usage,
                            tool_execution_status: Some(ToolExecutionStatus::ToolsComplete {
                                results: result_summaries,
                            }),
                            iteration,
                        });

                        // Store tool results as user message
                        let tool_result_message = Message {
                            id: 0,
                            conversation_id: conv_id.clone(),
                            created_at: chrono::Utc::now().timestamp(),
                            role: Role::User,
                            text_content: None,
                            content: tool_results,
                            input_tokens: None,
                            output_tokens: None,
                            model: None,
                        };

                        if let Err(e) = storage.add_message(&tool_result_message).await {
                            yield Err(e);
                            return;
                        }

                        // Continue loop to send tool results back
                    }
                    Some(StopReason::EndTurn) | Some(StopReason::MaxTokens) | Some(StopReason::StopSequence) | None => {
                        // Natural end - break out of loop
                        break;
                    }
                }
            }

            // Update conversation timestamp
            conv_for_title.touch();
            if let Err(e) = storage.update_conversation(&conv_for_title).await {
                yield Err(e);
                return;
            }

            // Generate title if needed (don't block on this)
            if conv_for_title.title.is_none() && iteration == 1
                && let Some(user_input) = user_input {
                    // Get the final message to extract text
                    if let Ok(messages) = storage.get_messages(&conv_id).await
                        && let Some(last_msg) = messages.last() {
                            let assistant_text = last_msg.text().unwrap_or("");
                            if let Ok(title) = title_generator.generate_title(&user_input, assistant_text).await {
                                info!("Generated title: {}", title);
                                conv_for_title.set_title(&title);
                                let _ = storage.update_conversation(&conv_for_title).await;
                            }
                        }
                }
        };

        Ok(Box::pin(stream))
    }

    // Helper function to build system prompt (extracted for reuse)
    fn build_system_prompt(config: &Config, conversation: &Conversation) -> String {
        let base_prompt = conversation.system_prompt.as_deref().unwrap_or_else(|| {
            if config.system_prompt.use_default {
                Config::default_system_prompt()
            } else {
                config.system_prompt.custom_prompt.as_deref().unwrap_or("")
            }
        });

        let instructions = conversation
            .custom_instructions
            .as_ref()
            .or(config.system_prompt.custom_instructions.as_ref());

        if let Some(inst) = instructions {
            format!("{}\n\n{}", base_prompt, inst)
        } else {
            base_prompt.to_string()
        }
    }

    // Static version of execute_tools for use in async_stream
    async fn execute_tools_static(
        message: &Message,
        executor: &ToolExecutor,
        conversation_id: &ConversationId,
        message_id: i64,
        config: &Config,
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

        let timeout = Duration::from_secs(config.tools.default_timeout);
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

    // Helper to create error message for tool execution failures
    fn create_tool_error_message(conv_id: &ConversationId, error: &crate::error::Error) -> Message {
        Message {
            id: 0,
            conversation_id: conv_id.clone(),
            created_at: chrono::Utc::now().timestamp(),
            role: Role::User,
            text_content: Some(format!("Tool execution error: {}", error)),
            content: vec![ContentBlock::Text {
                text: format!("Tool execution failed with error: {}", error),
            }],
            input_tokens: None,
            output_tokens: None,
            model: None,
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
        let messages = self.storage.get_messages(&conversation.id).await?;

        // Build the request
        let system_prompt = self.get_effective_system_prompt(conversation);

        let request = ChatRequest::new(messages, &conversation.model)
            .with_system_prompt(system_prompt)
            .with_max_tokens(4096);

        // Send to backend
        let response = self.backend.send_message(&request).await?;

        // Store assistant response
        let mut assistant_message = response.message.clone();
        assistant_message.conversation_id = conversation.id.clone();
        self.storage.add_message(&assistant_message).await?;

        // Update conversation
        conversation.touch();
        self.storage.update_conversation(conversation).await?;

        Ok(response)
    }

    /// Execute all tool uses in a message
    async fn execute_tools(
        &self,
        message: &Message,
        executor: &ToolExecutor,
        conversation_id: &ConversationId,
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
    pub async fn refresh_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.storage.list_conversations().await
    }

    /// Search conversations
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.storage.search(query).await
    }

    /// Delete a conversation
    pub async fn delete_conversation(&self, id: &ConversationId) -> Result<()> {
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
        Self::build_system_prompt(&self.config, conversation)
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
