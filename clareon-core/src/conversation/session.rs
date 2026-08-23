// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-conversation session with in-memory message cache

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::{Stream, StreamExt};
use tokio::sync::{Mutex as TokioMutex, RwLock, oneshot, watch};
use tracing::{debug, info, warn};

use crate::backend::{ChatRequest, ContentDelta, LlmBackend, StopReason, StreamEvent, Usage};
use crate::config::{Config, ConfigManager};
use crate::conversation::title::TitleGenerator;
use crate::error::Result;
use crate::storage::Storage;
use crate::tools::{AlwaysAllowRule, ToolExecutor};
use crate::types::{ContentBlock, Conversation, ConversationId, Message, Role, ToolResultContent};

use super::manager::{PendingToolUse, StreamUpdate, ToolApprovalDecision, ToolExecutionStatus};

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
    cancel_tx: watch::Sender<bool>,
    approval_tx: TokioMutex<Option<oneshot::Sender<ToolApprovalDecision>>>,
    pending_tools: TokioMutex<Vec<PendingToolUse>>,
    generating: AtomicBool,
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
        let (cancel_tx, _) = watch::channel(false);
        Self {
            conv_id,
            conversation: RwLock::new(conversation),
            backend,
            storage,
            messages: RwLock::new(messages),
            config,
            title_generator,
            tool_executor,
            cancel_tx,
            approval_tx: TokioMutex::new(None),
            pending_tools: TokioMutex::new(Vec::new()),
            generating: AtomicBool::new(false),
        }
    }

    pub fn is_generating(&self) -> bool {
        self.generating.load(Ordering::SeqCst)
    }

    /// Stop an in-flight generation, including a pending tool-approval prompt.
    pub fn cancel(&self) {
        self.cancel_tx.send_replace(true);
        if let Ok(mut slot) = self.approval_tx.try_lock()
            && let Some(tx) = slot.take()
        {
            let _ = tx.send(ToolApprovalDecision::Cancelled);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancel_tx.borrow()
    }

    /// Resolve the current tool-approval prompt. No-op if none is pending.
    pub async fn submit_tool_approval(&self, decision: ToolApprovalDecision) {
        if let Some(tx) = self.approval_tx.lock().await.take() {
            let _ = tx.send(decision);
        }
    }

    fn reset_generation_control(&self) {
        self.cancel_tx.send_replace(false);
    }

    async fn wait_if_cancelled<T>(&self, fut: impl std::future::Future<Output = T>) -> Option<T> {
        let mut rx = self.cancel_tx.subscribe();
        if *rx.borrow() {
            return None;
        }
        tokio::select! {
            () = async {
                while rx.changed().await.is_ok() {
                    if *rx.borrow() {
                        return;
                    }
                }
            } => None,
            result = fut => Some(result),
        }
    }

    async fn wait_for_approval(&self) -> ToolApprovalDecision {
        let (tx, rx) = oneshot::channel();
        *self.approval_tx.lock().await = Some(tx);
        let decision = tokio::select! {
            result = rx => result.unwrap_or(ToolApprovalDecision::Cancelled),
            () = async {
                let mut cancel_rx = self.cancel_tx.subscribe();
                if *cancel_rx.borrow() {
                    return;
                }
                while cancel_rx.changed().await.is_ok() {
                    if *cancel_rx.borrow() {
                        return;
                    }
                }
            } => ToolApprovalDecision::Cancelled,
        };
        *self.approval_tx.lock().await = None;
        decision
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
            if session.generating.swap(true, Ordering::SeqCst) {
                yield Err(crate::error::Error::GenerationInProgress);
                return;
            }
            session.reset_generation_control();
            struct GeneratingGuard<'a>(&'a AtomicBool);
            impl Drop for GeneratingGuard<'_> {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _generating = GeneratingGuard(&session.generating);

            let mut iteration = 0;

            loop {
                iteration += 1;

                if session.is_cancelled() {
                    yield cancelled_update(iteration);
                    return;
                }

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
                loop {
                    let next = session.wait_if_cancelled(backend_stream.next()).await;
                    let Some(result) = next else {
                        if let Some(msg) = save_partial_assistant(
                            &session,
                            &content_blocks,
                            &model,
                            usage,
                        )
                        .await
                        {
                            match msg {
                                Ok(()) => {}
                                Err(e) => {
                                    yield Err(e);
                                    return;
                                }
                            }
                        }
                        yield cancelled_update(iteration);
                        return;
                    };
                    let Some(result) = result else {
                        break;
                    };
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
                                cancelled: false,
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

                        let pending = pending_from_blocks(&content_blocks);
                        let tools_cfg = ConfigManager::get().config().tools;
                        let (pre_denied, remaining) =
                            split_denied(&tools_cfg.always_deny, &pending);
                        let mut tool_results = rejection_results(
                            &pre_denied,
                            "Blocked by always-deny policy",
                        );

                        let skipped_results = if remaining.is_empty() {
                            Some(Vec::new())
                        } else if pending_needs_approval(
                            tools_cfg.auto_execute,
                            &tools_cfg.always_allow,
                            &tools_cfg.always_deny,
                            &remaining,
                        ) {
                            *session.pending_tools.lock().await = remaining.clone();
                            yield Ok(StreamUpdate {
                                event: StreamEvent::MessageStop {
                                    stop_reason: StopReason::ToolUse,
                                },
                                partial_content: content_blocks.clone(),
                                stop_reason: Some(StopReason::ToolUse),
                                usage,
                                tool_execution_status: Some(
                                    ToolExecutionStatus::PendingApproval {
                                        tools: remaining.clone(),
                                    },
                                ),
                                iteration,
                                cancelled: false,
                            });

                            match session.wait_for_approval().await {
                                ToolApprovalDecision::AllowOnce => {
                                    session.pending_tools.lock().await.clear();
                                    None
                                }
                                ToolApprovalDecision::AlwaysAllow => {
                                    persist_policy_rules(&remaining, true);
                                    session.pending_tools.lock().await.clear();
                                    None
                                }
                                ToolApprovalDecision::AlwaysDeny => {
                                    persist_policy_rules(&remaining, false);
                                    session.pending_tools.lock().await.clear();
                                    Some(rejection_results(
                                        &remaining,
                                        "Blocked by always-deny policy",
                                    ))
                                }
                                ToolApprovalDecision::Deny => {
                                    session.pending_tools.lock().await.clear();
                                    Some(rejection_results(
                                        &remaining,
                                        "User denied tool execution",
                                    ))
                                }
                                ToolApprovalDecision::Cancelled => {
                                    session.pending_tools.lock().await.clear();
                                    if let Err(e) = store_tool_results(
                                        &session,
                                        {
                                            let mut cancelled = tool_results.clone();
                                            cancelled.extend(rejection_results(
                                                &remaining,
                                                "Generation stopped by user",
                                            ));
                                            cancelled
                                        },
                                    )
                                    .await
                                    {
                                        yield Err(e);
                                    } else {
                                        yield cancelled_update(iteration);
                                    }
                                    return;
                                }
                            }
                        } else {
                            None
                        };

                        let executed = if let Some(denied) = skipped_results {
                            denied
                        } else {
                            if session.is_cancelled() {
                                if let Err(e) = store_tool_results(
                                    &session,
                                    rejection_results(&remaining, "Generation stopped by user"),
                                )
                                .await
                                {
                                    yield Err(e);
                                } else {
                                    yield cancelled_update(iteration);
                                }
                                return;
                            }

                            yield Ok(StreamUpdate {
                                event: StreamEvent::MessageStop {
                                    stop_reason: StopReason::ToolUse,
                                },
                                partial_content: content_blocks.clone(),
                                stop_reason: Some(StopReason::ToolUse),
                                usage,
                                tool_execution_status: Some(
                                    ToolExecutionStatus::ExecutingTools { count: tool_count },
                                ),
                                iteration,
                                cancelled: false,
                            });

                            info!(
                                "Executing {} tools (iteration {})",
                                remaining.len(),
                                iteration
                            );
                            match session
                                .wait_if_cancelled(Self::execute_pending_tools(
                                    &remaining,
                                    executor,
                                    &session.conv_id,
                                    assistant_msg_id,
                                    &session.config,
                                ))
                                .await
                            {
                                None => {
                                    if let Err(e) = store_tool_results(
                                        &session,
                                        rejection_results(
                                            &remaining,
                                            "Generation stopped by user",
                                        ),
                                    )
                                    .await
                                    {
                                        yield Err(e);
                                    } else {
                                        yield cancelled_update(iteration);
                                    }
                                    return;
                                }
                                Some(Err(e)) => {
                                    yield Ok(StreamUpdate {
                                        event: StreamEvent::MessageStop {
                                            stop_reason: StopReason::ToolUse,
                                        },
                                        partial_content: content_blocks.clone(),
                                        stop_reason: Some(StopReason::ToolUse),
                                        usage,
                                        tool_execution_status: Some(
                                            ToolExecutionStatus::ToolError {
                                                error: e.to_string(),
                                            },
                                        ),
                                        iteration,
                                        cancelled: false,
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
                                Some(Ok(results)) => results,
                            }
                        };
                        tool_results.extend(executed);

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
                            cancelled: false,
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
            if !session.is_cancelled()
                && needs_title
                && iteration == 1
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

    async fn execute_pending_tools(
        pending: &[PendingToolUse],
        executor: &ToolExecutor,
        conversation_id: &ConversationId,
        message_id: i64,
        config: &Config,
    ) -> Result<Vec<ContentBlock>> {
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        let tool_uses: Vec<(&str, &str, &serde_json::Value)> = pending
            .iter()
            .map(|tool| (tool.id.as_str(), tool.name.as_str(), &tool.input))
            .collect();
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

fn cancelled_update(iteration: usize) -> Result<StreamUpdate> {
    Ok(StreamUpdate {
        event: StreamEvent::MessageStop {
            stop_reason: StopReason::EndTurn,
        },
        partial_content: Vec::new(),
        stop_reason: Some(StopReason::EndTurn),
        usage: Usage::default(),
        tool_execution_status: None,
        iteration,
        cancelled: true,
    })
}

fn has_meaningful_content(blocks: &[ContentBlock]) -> bool {
    blocks.iter().any(|block| match block {
        ContentBlock::Text { text } => !text.trim().is_empty(),
        ContentBlock::ToolUse { .. } => true,
        ContentBlock::Image { .. } => true,
        ContentBlock::ToolResult { .. } => true,
    })
}

async fn save_partial_assistant(
    session: &ConversationSession,
    content_blocks: &[ContentBlock],
    model: &str,
    usage: Usage,
) -> Option<Result<()>> {
    if !has_meaningful_content(content_blocks) {
        return None;
    }
    let assistant_msg = Message::assistant(
        session.conv_id.clone(),
        content_blocks.to_vec(),
        model,
        usage.input_tokens,
        usage.output_tokens,
    );
    Some(
        async {
            let id = session.storage.add_message(&assistant_msg).await?;
            let mut cached = session.messages.write().await;
            let mut stored = assistant_msg;
            stored.id = id;
            cached.push(stored);
            Ok(())
        }
        .await,
    )
}

async fn store_tool_results(
    session: &ConversationSession,
    tool_results: Vec<ContentBlock>,
) -> Result<()> {
    if tool_results.is_empty() {
        return Ok(());
    }
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
    let tool_msg_id = session.storage.add_message(&tool_result_message).await?;
    let mut cached = session.messages.write().await;
    let mut stored = tool_result_message;
    stored.id = tool_msg_id;
    cached.push(stored);
    Ok(())
}

fn pending_from_blocks(blocks: &[ContentBlock]) -> Vec<PendingToolUse> {
    blocks
        .iter()
        .filter_map(|block| {
            if let ContentBlock::ToolUse { id, name, input } = block {
                let always_label = AlwaysAllowRule::from_invocation(name, input).map(|r| r.label());
                Some(PendingToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                    always_label,
                })
            } else {
                None
            }
        })
        .collect()
}

fn pending_needs_approval(
    auto_execute: bool,
    always_allow: &[String],
    always_deny: &[String],
    pending: &[PendingToolUse],
) -> bool {
    let calls: Vec<(String, serde_json::Value)> = pending
        .iter()
        .map(|tool| (tool.name.clone(), tool.input.clone()))
        .collect();
    crate::tools::tools_need_approval(auto_execute, always_allow, always_deny, &calls)
}

fn split_denied(
    always_deny: &[String],
    pending: &[PendingToolUse],
) -> (Vec<PendingToolUse>, Vec<PendingToolUse>) {
    let mut denied = Vec::new();
    let mut remaining = Vec::new();
    for tool in pending {
        if crate::tools::is_denied(always_deny, &tool.name, &tool.input) {
            denied.push(tool.clone());
        } else {
            remaining.push(tool.clone());
        }
    }
    (denied, remaining)
}

fn rejection_results(pending: &[PendingToolUse], reason: &str) -> Vec<ContentBlock> {
    pending
        .iter()
        .map(|tool| {
            ContentBlock::tool_result(tool.id.clone(), vec![ToolResultContent::text(reason)], true)
        })
        .collect()
}

fn persist_policy_rules(pending: &[PendingToolUse], allow: bool) {
    let specs: Vec<String> = pending
        .iter()
        .filter_map(|tool| AlwaysAllowRule::from_invocation(&tool.name, &tool.input))
        .map(|rule| rule.to_spec())
        .collect();
    if specs.is_empty() {
        return;
    }
    if let Err(e) = ConfigManager::get().update_config(|config| {
        let list = if allow {
            &mut config.tools.always_allow
        } else {
            &mut config.tools.always_deny
        };
        for spec in &specs {
            if !list.iter().any(|existing| existing == spec) {
                list.push(spec.clone());
            }
        }
    }) {
        warn!("Failed to update tool policy list: {e}");
        return;
    }
    if let Err(e) = ConfigManager::get().save() {
        warn!("Failed to persist tool policy list: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(name: &str) -> PendingToolUse {
        PendingToolUse {
            id: format!("id_{name}"),
            name: name.to_string(),
            input: serde_json::json!({"arg": 1}),
            always_label: None,
        }
    }

    #[test]
    fn approval_skipped_when_auto_execute() {
        assert!(!pending_needs_approval(
            true,
            &[],
            &[],
            &[pending("read_file"), pending("write_file")]
        ));
    }

    #[test]
    fn approval_required_when_auto_execute_off() {
        assert!(pending_needs_approval(
            false,
            &[],
            &[],
            &[pending("read_file")]
        ));
    }

    #[test]
    fn bare_tool_name_does_not_allow_path_tools() {
        let read = PendingToolUse {
            id: "1".into(),
            name: "read_file".into(),
            input: serde_json::json!({"path": "/tmp/a"}),
            always_label: None,
        };
        assert!(pending_needs_approval(
            false,
            &["read_file".into()],
            &[],
            &[read]
        ));
    }

    #[test]
    fn rejection_results_mark_error() {
        let results = rejection_results(&[pending("read_file")], "denied");
        assert_eq!(results.len(), 1);
        match &results[0] {
            ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                content,
            } => {
                assert_eq!(tool_use_id, "id_read_file");
                assert_eq!(*is_error, Some(true));
                assert_eq!(content, &vec![ToolResultContent::text("denied")]);
            }
            other => panic!("unexpected block: {other:?}"),
        }
    }

    #[test]
    fn empty_text_is_not_meaningful() {
        assert!(!has_meaningful_content(&[ContentBlock::text("   ")]));
        assert!(has_meaningful_content(&[ContentBlock::text("hi")]));
        assert!(has_meaningful_content(&[ContentBlock::tool_use(
            "1",
            "read_file",
            serde_json::json!({})
        )]));
    }
}
