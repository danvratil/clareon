// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application state management

use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc;

use ratatui::widgets::ListState;

use clareon_core::{
    ArtifactManager, BedrockBackend, BubblewrapSandbox, Config, ConversationManager, LlmBackend,
    NoneSandbox, Sandbox, SandboxMode, SandboxModeConfig, Storage, StreamUpdate, ToolExecutor,
    ToolRegistry, WorkspaceManager,
    backend::Usage,
    config::Backend,
    register_builtin_tools,
    types::{
        ContentBlock, Conversation, ConversationId, ConversationSummary, Message, SearchResult,
    },
};

/// Current view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Main chat view
    Chat,
    /// Conversation list
    ConversationList,
    /// Search results
    #[allow(dead_code)]
    SearchResults,
    /// Help screen
    Help,
}

/// Partial message being streamed
#[derive(Debug, Clone)]
pub struct PartialMessage {
    /// Content accumulated so far
    pub content: Vec<ContentBlock>,
    /// Token usage
    pub usage: Usage,
}

/// Result type for streaming updates
pub type StreamResult = Result<StreamUpdate, anyhow::Error>;

/// Application state
pub struct App {
    /// Conversation manager (wrapped in Arc for cloning into background tasks)
    pub manager: Arc<ConversationManager>,

    /// Current view mode
    pub view_mode: ViewMode,

    /// Current conversation (if any)
    pub conversation: Option<Conversation>,

    /// Messages in current conversation
    pub messages: Vec<Message>,

    /// List of conversations (for list view)
    pub conversations: Vec<ConversationSummary>,

    /// Search results
    pub search_results: Vec<SearchResult>,

    /// Current search query
    pub search_query: String,

    /// Input buffer
    pub input: String,

    /// Scroll state for messages list
    pub message_list_state: ListState,

    /// Is the app running?
    pub running: bool,

    /// Status message
    pub status: Option<String>,

    /// Is waiting for response?
    pub waiting: bool,

    /// Configuration
    pub config: Config,

    /// Streaming message in progress (if any)
    pub streaming_message: Option<PartialMessage>,

    /// Channel receiver for streaming updates
    pub stream_rx: Option<mpsc::UnboundedReceiver<StreamResult>>,

    /// Flag to reload messages after streaming completes
    pub needs_reload: bool,

    /// Last message usage (for status bar display)
    pub last_usage: Option<Usage>,

    /// Cumulative conversation usage
    pub conversation_usage: Usage,
}

/// Options for creating the app
pub struct AppOptions {
    /// AWS profile to use
    pub profile: Option<String>,
    /// AWS region override
    pub region: Option<String>,
}

impl App {
    /// Create a new application instance
    pub async fn new(config: Config, options: AppOptions) -> anyhow::Result<Self> {
        // Initialize storage
        let db_url = Config::database_url()?;
        let storage = Storage::new(&db_url).await?;

        // Determine region (CLI override > config)
        let region = options
            .region
            .as_ref()
            .unwrap_or(&config.backends.bedrock.region)
            .clone();

        // Initialize backend based on config
        let backend: Arc<dyn LlmBackend> = match config.default_backend {
            Backend::Anthropic => {
                // For now, require API key in environment
                let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                    anyhow::anyhow!(
                        "ANTHROPIC_API_KEY environment variable not set. \
                         Set it or use --backend bedrock"
                    )
                })?;
                Arc::new(clareon_core::AnthropicBackend::new(api_key))
            }
            Backend::Bedrock => {
                // Default to Bedrock - use profile if specified
                let profile = options
                    .profile
                    .clone()
                    .or_else(|| config.backends.bedrock.profile.clone());
                let enable_caching = config.backends.bedrock.enable_prompt_caching;

                Arc::new(BedrockBackend::new_with_config(&region, profile, enable_caching).await?)
            }
        };

        // Initialize tool executor if tools are enabled
        let mut manager =
            ConversationManager::with_single_backend(storage, backend, config.clone());

        if config.tools.enabled {
            // Get cache root directory
            let cache_root = Config::cache_root()?;

            // Get storage Arc for workspace manager
            let storage_arc = manager.storage();

            // Create workspace manager
            let workspace_manager =
                Arc::new(WorkspaceManager::new(cache_root, storage_arc.clone()));

            // Ensure shared directories exist
            workspace_manager.ensure_shared_directories().await?;

            // Create artifact manager
            let artifact_manager = Arc::new(ArtifactManager::new(storage_arc.clone()));

            // Create tool registry and register built-in tools
            let mut registry = ToolRegistry::default();
            register_builtin_tools(&mut registry);

            // Create sandbox based on config
            let sandbox: Arc<dyn Sandbox> = match config.tools.sandbox_mode {
                SandboxModeConfig::None => Arc::new(NoneSandbox),
                SandboxModeConfig::Basic => Arc::new(BubblewrapSandbox::new(SandboxMode::Basic)),
                SandboxModeConfig::Strict => Arc::new(BubblewrapSandbox::new(SandboxMode::Strict)),
            };

            // Create tool executor with workspace and artifact managers
            let executor = ToolExecutor::new(
                Arc::new(registry),
                sandbox,
                workspace_manager,
                artifact_manager,
            );

            // Add tools to conversation manager
            manager = manager.with_tools(Arc::new(executor));
        }

        Ok(Self {
            manager: Arc::new(manager),
            view_mode: ViewMode::Chat,
            conversation: None,
            messages: Vec::new(),
            conversations: Vec::new(),
            search_results: Vec::new(),
            search_query: String::new(),
            input: String::new(),
            message_list_state: ListState::default(),
            running: true,
            status: None,
            waiting: false,
            config,
            streaming_message: None,
            stream_rx: None,
            needs_reload: false,
            last_usage: None,
            conversation_usage: Usage::default(),
        })
    }

    /// Start a new conversation
    pub async fn new_conversation(&mut self) -> anyhow::Result<()> {
        let conv = self.manager.new_conversation().await?;
        self.conversation = Some(conv);
        self.messages.clear();
        self.message_list_state = ListState::default();
        self.last_usage = None;
        self.conversation_usage = Usage::default();
        self.status = Some("New conversation started".to_string());
        Ok(())
    }

    /// Load a conversation by ID
    pub async fn load_conversation(&mut self, id: &ConversationId) -> anyhow::Result<()> {
        let conv = self.manager.load_conversation(id).await?;
        let messages = self.manager.get_messages(id).await?;

        self.conversation = Some(conv);
        self.messages = messages;
        self.message_list_state = ListState::default();
        self.calculate_conversation_usage();
        self.status = Some(format!("Loaded conversation {}", id));

        Ok(())
    }

    /// Send a message with streaming
    pub async fn send_message(&mut self) -> anyhow::Result<()> {
        if self.input.trim().is_empty() {
            return Ok(());
        }

        // Ensure we have a conversation
        if self.conversation.is_none() {
            self.new_conversation().await?;
        }

        let user_input = self.input.clone();
        self.input.clear();

        // Add user message to UI immediately
        let conv_id = self.conversation.as_ref().unwrap().id.clone();
        let user_message = clareon_core::types::Message::user(conv_id, &user_input);
        self.messages.push(user_message);
        self.scroll_to_bottom();

        self.waiting = true;

        // If tools are enabled, use non-streaming version for now
        // (streaming + tools requires more complex handling)
        if self.config.tools.enabled {
            self.status = Some("Processing with tools enabled...".to_string());

            let mut conv = self.conversation.clone().unwrap();
            let manager = Arc::clone(&self.manager);

            // Spawn background task for non-streaming with tools
            let (tx, rx) = mpsc::unbounded_channel();
            self.stream_rx = Some(rx);

            tokio::spawn(async move {
                match manager
                    .send_message_with_tools(&mut conv, &user_input)
                    .await
                {
                    Ok(response) => {
                        // Send the final response as a stream update
                        let update = StreamUpdate {
                            event: clareon_core::backend::StreamEvent::MessageStop {
                                stop_reason: response.stop_reason,
                            },
                            partial_content: response.message.content.clone(),
                            stop_reason: Some(response.stop_reason),
                            usage: response.usage,
                        };
                        let _ = tx.send(Ok(update));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                    }
                }
            });
        } else {
            // Use streaming for non-tool messages
            self.streaming_message = Some(PartialMessage {
                content: Vec::new(),
                usage: Usage::default(),
            });

            // Create channel for streaming updates
            let (tx, rx) = mpsc::unbounded_channel();
            self.stream_rx = Some(rx);

            // Clone for background task
            let mut conv = self.conversation.clone().unwrap();
            let manager = Arc::clone(&self.manager);

            // Spawn background task to handle streaming
            tokio::spawn(async move {
                match manager.send_message_stream(&mut conv, &user_input).await {
                    Ok(mut stream) => {
                        while let Some(update) = stream.next().await {
                            if tx.send(update.map_err(Into::into)).is_err() {
                                // Receiver dropped - user cancelled or quit
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                    }
                }
            });
        }

        Ok(())
    }

    /// Load conversation list
    pub async fn load_conversation_list(&mut self) -> anyhow::Result<()> {
        self.conversations = self.manager.refresh_conversations().await?;
        self.view_mode = ViewMode::ConversationList;
        Ok(())
    }

    /// Search conversations
    #[allow(dead_code)]
    pub async fn search(&mut self, query: &str) -> anyhow::Result<()> {
        self.search_query = query.to_string();
        self.search_results = self.manager.search(query).await?;
        self.view_mode = ViewMode::SearchResults;
        Ok(())
    }

    /// Scroll messages up
    pub fn scroll_up(&mut self, amount: usize) {
        let current = self.message_list_state.selected().unwrap_or(0);
        let new_pos = current.saturating_sub(amount);
        self.message_list_state.select(Some(new_pos));
    }

    /// Scroll messages down
    pub fn scroll_down(&mut self, amount: usize) {
        // Calculate total items (messages + streaming message if present)
        let total_items = self.messages.len()
            + if self.streaming_message.is_some() {
                1
            } else {
                0
            };

        if total_items == 0 {
            return;
        }

        let current = self.message_list_state.selected().unwrap_or(0);
        let new_pos = (current + amount).min(total_items.saturating_sub(1));
        self.message_list_state.select(Some(new_pos));
    }

    /// Scroll to the bottom of messages
    pub fn scroll_to_bottom(&mut self) {
        let total_items = self.messages.len()
            + if self.streaming_message.is_some() {
                1
            } else {
                0
            };
        if total_items > 0 {
            self.message_list_state
                .select(Some(total_items.saturating_sub(1)));
        }
    }

    /// Calculate cumulative usage from all messages in the conversation
    pub fn calculate_conversation_usage(&mut self) {
        let mut total_input = 0i64;
        let mut total_output = 0i64;
        let total_cache_read = 0i64;
        let total_cache_write = 0i64;

        for message in &self.messages {
            if let Some(tokens) = message.input_tokens {
                total_input += tokens;
            }
            if let Some(tokens) = message.output_tokens {
                total_output += tokens;
            }
        }

        // Note: Cache metrics aren't currently stored in the database,
        // so we can only show the last message's cache metrics
        self.conversation_usage = Usage {
            input_tokens: total_input,
            output_tokens: total_output,
            cache_read_input_tokens: if total_cache_read > 0 {
                Some(total_cache_read)
            } else {
                None
            },
            cache_write_input_tokens: if total_cache_write > 0 {
                Some(total_cache_write)
            } else {
                None
            },
        };
    }

    /// Quit the application
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Get current conversation title
    pub fn conversation_title(&self) -> &str {
        self.conversation
            .as_ref()
            .map(|c| c.display_title())
            .unwrap_or("New Chat")
    }
}
