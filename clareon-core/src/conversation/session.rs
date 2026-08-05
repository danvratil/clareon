// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-conversation session with in-memory message cache

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{Stream, StreamExt};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::backend::{ChatRequest, ContentDelta, LlmBackend, StopReason, StreamEvent, Usage};
use crate::config::Config;
use crate::conversation::title::TitleGenerator;
use crate::error::Result;
use crate::storage::Storage;
use crate::tools::ToolExecutor;
use crate::types::{ContentBlock, Conversation, ConversationId, Message, Role};

use super::manager::{StreamUpdate, ToolExecutionStatus};

/// Per-conversation session with a cached message history and its own backend instance.
///
/// Sessions are created lazily when a conversation is first accessed and live for
/// the duration of the application's use of that conversation. Each session owns
/// an in-memory copy of the conversation's message history, eliminating repeated
/// database reads during multi-turn exchanges and tool execution loops.
pub struct ConversationSession {
    conv_id: ConversationId,
    conversation: RwLock<Conversation>,
    backend: Arc<dyn LlmBackend>,
    storage: Arc<Storage>,
    messages: RwLock<Vec<Message>>,
    config: Config,
    title_generator: Arc<TitleGenerator>,
    tool_executor: Option<Arc<ToolExecutor>>,
}

impl ConversationSession {
    pub fn new_with_messages(
        conversation: Conversation,
        backend: Arc<dyn LlmBackend>,
        storage: Arc<Storage>,
        messages: Vec<Message>,
        config: Config,
        title_generator: Arc<TitleGenerator>,
        tool_executor: Option<Arc<ToolExecutor>>,
    ) -> Self {
        let conv_id = conversation.id.clone();
        Self {
            conv_id,
            conversation: RwLock::new(conversation),
            backend,
            storage,
            messages: RwLock::new(messages),
            config,
            title_generator,
            tool_executor,
        }
    }

    pub fn conversation_id(&self) -> &ConversationId {
        &self.conv_id
    }

    pub async fn get_conversation(&self) -> Conversation {
        self.conversation.read().await.clone()
    }

    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }

    pub async fn append_user_message(&self, text: &str) -> Result<Message> {
        let mut msg = Message::user(self.conv_id.clone(), text);
        let id = self.storage.add_message(&msg).await?;
        msg.id = id;
        debug!("Stored user message: {}", id);
        self.messages.write().await.push(msg.clone());
        Ok(msg)
    }

    pub async fn append_user_message_with_content(
        &self,
        content: Vec<ContentBlock>,
    ) -> Result<Message> {
        let mut msg = Message::user_with_content(self.conv_id.clone(), content);
        let id = self.storage.add_message(&msg).await?;
        msg.id = id;
        debug!("Stored user message with content: {}", id);
        self.messages.write().await.push(msg.clone());
        Ok(msg)
    }

    /// Stream a response for the current conversation state.
    ///
    /// Uses the in-memory message cache instead of querying the database on each
    /// tool iteration. New messages (assistant replies, tool results) are appended
    /// to both the cache and the database as they arrive.
    pub async fn send_message_stream(
        self: &Arc<Self>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate>> + Send>>> {
        const MAX_TOOL_ITERATIONS: usize = 5;

        let user_input = {
            let msgs = self.messages.read().await;
            msgs.iter()
                .rev()
                .find(|m| m.role == Role::User)
                .and_then(|m| m.text())
                .map(|s| s.to_string())
        };

        let needs_title = self.conversation.read().await.title.is_none();

        let session = Arc::clone(self);

        let stream = async_stream::stream! {
            let mut iteration = 0;

            loop {
                iteration += 1;

                if iteration > MAX_TOOL_ITERATIONS {
                    yield Err(crate::error::Error::Tool(
                        crate::tools::ToolError::ExecutionFailed(
                            format!("Too many tool iterations (max: {})", MAX_TOOL_ITERATIONS),
                        ),
                    ));
                    return;
                }

                // Read from cache - no database query needed
                let messages = session.messages.read().await.clone();

                // Use the live configured default model so settings changes apply
                // without restart. Keep conversation.model in sync for metadata/UI.
                let model = session.config.default_model.clone();
                {
                    let mut conv = session.conversation.write().await;
                    if conv.model != model {
                        conv.model = model.clone();
                        conv.touch();
                        let conv_snapshot = conv.clone();
                        drop(conv);
                        if let Err(e) = session.storage.update_conversation(&conv_snapshot).await {
                            warn!("Failed to sync conversation model: {}", e);
                        }
                    }
                }
                let system_prompt = {
                    let conv = session.conversation.read().await;
                    Self::build_system_prompt(&session.config, &conv)
                };

                let mut request = ChatRequest::new(messages, &model)
                    .with_system_prompt(system_prompt)
                    .with_max_tokens(4096);

                if let Some(ref executor) = session.tool_executor {
                    let tools = executor.registry.tool_definitions();
                    debug!(
                        "Adding {} tools to request (iteration {})",
                        tools.len(),
                        iteration
                    );
                    request.tools = tools;
                } else {
                    debug!("Not adding tools - no tool executor");
                }

                info!(
                    "Streaming request to {} backend (iteration {})",
                    session.backend.name(),
                    iteration
                );
                let backend_stream = match session.backend.send_message_stream(&request).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        yield Err(e.into());
                        return;
                    }
                };

                let mut content_blocks: Vec<ContentBlock> = Vec::new();
                let mut stop_reason: Option<StopReason> = None;
                let mut usage = Usage::default();
                let mut tool_input_accumulators: HashMap<usize, String> = HashMap::new();

                let mut backend_stream = Box::pin(backend_stream);
                while let Some(result) = backend_stream.next().await {
                    match result {
                        Ok(event) => {
                            match &event {
                                StreamEvent::ContentBlockStart { index, block } => {
                                    while content_blocks.len() <= *index {
                                        content_blocks
                                            .push(ContentBlock::Text { text: String::new() });
                                    }
                                    content_blocks[*index] = block.clone();

                                    if matches!(block, ContentBlock::ToolUse { .. }) {
                                        tool_input_accumulators.insert(*index, String::new());
                                    }
                                }
                                StreamEvent::ContentBlockDelta { index, delta } => {
                                    while content_blocks.len() <= *index {
                                        content_blocks
                                            .push(ContentBlock::Text { text: String::new() });
                                    }

                                    match delta {
                                        ContentDelta::Text { text: delta_text } => {
                                            if let Some(ContentBlock::Text { text }) =
                                                content_blocks.get_mut(*index)
                                            {
                                                text.push_str(delta_text);
                                            }
                                        }
                                        ContentDelta::ToolInput { partial_json } => {
                                            tool_input_accumulators
                                                .entry(*index)
                                                .or_default()
                                                .push_str(partial_json);

                                            if let Some(ContentBlock::ToolUse { input, .. }) =
                                                content_blocks.get_mut(*index)
                                            {
                                                *input = serde_json::Value::String(
                                                    tool_input_accumulators[index].clone(),
                                                );
                                            }
                                        }
                                    }
                                }
                                StreamEvent::ContentBlockStop { index } => {
                                    if let Some(json_str) =
                                        tool_input_accumulators.remove(index)
                                        && let Some(ContentBlock::ToolUse { input, .. }) =
                                            content_blocks.get_mut(*index)
                                    {
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

                // Store assistant message and update the cache
                let assistant_msg = Message::assistant(
                    session.conv_id.clone(),
                    content_blocks.clone(),
                    &model,
                    usage.input_tokens,
                    usage.output_tokens,
                );

                let assistant_msg_id = match session.storage.add_message(&assistant_msg).await {
                    Ok(id) => id,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                {
                    let mut cached = session.messages.write().await;
                    let mut cached_msg = assistant_msg.clone();
                    cached_msg.id = assistant_msg_id;
                    cached.push(cached_msg);
                }

                match stop_reason {
                    Some(StopReason::ToolUse) => {
                        let tool_count =
                            content_blocks.iter().filter(|b| b.is_tool_use()).count();

                        if tool_count == 0 {
                            warn!("Stop reason is ToolUse but no tool use blocks found");
                            break;
                        }

                        let Some(ref executor) = session.tool_executor else {
                            yield Err(crate::error::Error::Tool(
                                crate::tools::ToolError::ExecutionFailed(
                                    "Model requested tools but executor not configured"
                                        .to_string(),
                                ),
                            ));
                            return;
                        };

                        yield Ok(StreamUpdate {
                            event: StreamEvent::MessageStop {
                                stop_reason: StopReason::ToolUse,
                            },
                            partial_content: content_blocks.clone(),
                            stop_reason: Some(StopReason::ToolUse),
                            usage,
                            tool_execution_status: Some(ToolExecutionStatus::ExecutingTools {
                                count: tool_count,
                            }),
                            iteration,
                        });

                        info!("Executing {} tools (iteration {})", tool_count, iteration);
                        let tool_results = match Self::execute_tools_static(
                            &assistant_msg,
                            executor,
                            &session.conv_id,
                            assistant_msg_id,
                            &session.config,
                        )
                        .await
                        {
                            Ok(results) => results,
                            Err(e) => {
                                yield Ok(StreamUpdate {
                                    event: StreamEvent::MessageStop {
                                        stop_reason: StopReason::ToolUse,
                                    },
                                    partial_content: content_blocks.clone(),
                                    stop_reason: Some(StopReason::ToolUse),
                                    usage,
                                    tool_execution_status: Some(ToolExecutionStatus::ToolError {
                                        error: e.to_string(),
                                    }),
                                    iteration,
                                });

                                let error_message =
                                    Self::create_tool_error_message(&session.conv_id, &e);
                                match session.storage.add_message(&error_message).await {
                                    Ok(err_msg_id) => {
                                        let mut cached = session.messages.write().await;
                                        let mut stored = error_message;
                                        stored.id = err_msg_id;
                                        cached.push(stored);
                                    }
                                    Err(store_err) => {
                                        yield Err(store_err);
                                        return;
                                    }
                                }
                                continue;
                            }
                        };

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

                        yield Ok(StreamUpdate {
                            event: StreamEvent::MessageStop {
                                stop_reason: StopReason::ToolUse,
                            },
                            partial_content: content_blocks,
                            stop_reason: Some(StopReason::ToolUse),
                            usage,
                            tool_execution_status: Some(ToolExecutionStatus::ToolsComplete {
                                results: result_summaries,
                            }),
                            iteration,
                        });

                        // Store tool results and update the cache
                        let tool_result_message = Message {
                            id: 0,
                            conversation_id: session.conv_id.clone(),
                            created_at: chrono::Utc::now().timestamp(),
                            role: Role::User,
                            text_content: None,
                            content: tool_results,
                            input_tokens: None,
                            output_tokens: None,
                            model: None,
                        };

                        match session.storage.add_message(&tool_result_message).await {
                            Ok(tool_msg_id) => {
                                let mut cached = session.messages.write().await;
                                let mut stored = tool_result_message;
                                stored.id = tool_msg_id;
                                cached.push(stored);
                            }
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                    }
                    Some(StopReason::EndTurn)
                    | Some(StopReason::MaxTokens)
                    | Some(StopReason::StopSequence)
                    | None => {
                        break;
                    }
                }
            }

            // Update conversation timestamp
            {
                let mut conv = session.conversation.write().await;
                conv.touch();
                if let Err(e) = session.storage.update_conversation(&conv).await {
                    yield Err(e);
                    return;
                }
            }

            // Generate title if this is the first turn and no title exists yet
            if needs_title && iteration == 1
                && let Some(ref user_input_str) = user_input
            {
                let last_msg_text = {
                    let msgs = session.messages.read().await;
                    msgs.last()
                        .and_then(|m| m.text())
                        .map(|s| s.to_string())
                        .unwrap_or_default()
                };
                match session
                    .title_generator
                    .generate_title(user_input_str, &last_msg_text)
                    .await
                {
                    Ok(title) => {
                        info!("Generated title: {}", title);
                        let mut conv = session.conversation.write().await;
                        conv.set_title(&title);
                        let _ = session.storage.update_conversation(&conv).await;
                    }
                    Err(e) => {
                        warn!("Failed to generate title: {}", e);
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

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
            () = tokio::time::sleep(timeout) => {
                Err(crate::error::Error::Tool(
                    crate::tools::ToolError::Timeout(timeout),
                ))
            }
        }
    }

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
}
