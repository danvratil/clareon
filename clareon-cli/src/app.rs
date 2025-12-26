//! Application state management

use std::sync::Arc;

use clareon_core::{
    types::{Conversation, ConversationSummary, Message, SearchResult},
    BedrockBackend, Config, ConversationManager, LlmBackend, Storage,
};

/// Current view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Main chat view
    Chat,
    /// Conversation list
    ConversationList,
    /// Search results
    SearchResults,
    /// Help screen
    Help,
}

/// Application state
pub struct App {
    /// Conversation manager
    pub manager: ConversationManager,

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

    /// Scroll offset for messages
    pub scroll_offset: usize,

    /// Is the app running?
    pub running: bool,

    /// Status message
    pub status: Option<String>,

    /// Is waiting for response?
    pub waiting: bool,

    /// Configuration
    pub config: Config,
}

impl App {
    /// Create a new application instance
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        // Initialize storage
        let db_url = Config::database_url()?;
        let storage = Storage::new(&db_url).await?;

        // Initialize backend based on config
        let backend: Arc<dyn LlmBackend> = match config.default_backend.as_str() {
            "anthropic" => {
                // For now, require API key in environment
                let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                    anyhow::anyhow!(
                        "ANTHROPIC_API_KEY environment variable not set. \
                         Set it or use --backend bedrock"
                    )
                })?;
                Arc::new(clareon_core::AnthropicBackend::new(api_key))
            }
            _ => {
                // Default to Bedrock
                let region = &config.backends.bedrock.region;
                Arc::new(BedrockBackend::new(region).await?)
            }
        };

        let manager = ConversationManager::with_single_backend(storage, backend, config.clone());

        Ok(Self {
            manager,
            view_mode: ViewMode::Chat,
            conversation: None,
            messages: Vec::new(),
            conversations: Vec::new(),
            search_results: Vec::new(),
            search_query: String::new(),
            input: String::new(),
            scroll_offset: 0,
            running: true,
            status: None,
            waiting: false,
            config,
        })
    }

    /// Start a new conversation
    pub async fn new_conversation(&mut self) -> anyhow::Result<()> {
        let conv = self.manager.new_conversation().await?;
        self.conversation = Some(conv);
        self.messages.clear();
        self.scroll_offset = 0;
        self.status = Some("New conversation started".to_string());
        Ok(())
    }

    /// Load a conversation by ID
    pub async fn load_conversation(&mut self, id: i64) -> anyhow::Result<()> {
        let conv = self.manager.load_conversation(id).await?;
        let messages = self.manager.get_messages(id).await?;

        self.conversation = Some(conv);
        self.messages = messages;
        self.scroll_offset = 0;
        self.status = Some(format!("Loaded conversation {}", id));

        Ok(())
    }

    /// Send a message
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
        self.waiting = true;
        self.status = Some("Sending...".to_string());

        // Get mutable reference to conversation
        let conv = self.conversation.as_mut().unwrap();

        // Send message
        match self.manager.send_message(conv, &user_input).await {
            Ok(response) => {
                // Reload messages
                self.messages = self.manager.get_messages(conv.id).await?;
                self.status = Some(format!(
                    "Tokens: {} in / {} out",
                    response.usage.input_tokens, response.usage.output_tokens
                ));

                // Scroll to bottom
                self.scroll_to_bottom();
            }
            Err(e) => {
                self.status = Some(format!("Error: {}", e));
            }
        }

        self.waiting = false;
        Ok(())
    }

    /// Load conversation list
    pub async fn load_conversation_list(&mut self) -> anyhow::Result<()> {
        self.conversations = self.manager.list_conversations().await?;
        self.view_mode = ViewMode::ConversationList;
        Ok(())
    }

    /// Search conversations
    pub async fn search(&mut self, query: &str) -> anyhow::Result<()> {
        self.search_query = query.to_string();
        self.search_results = self.manager.search(query).await?;
        self.view_mode = ViewMode::SearchResults;
        Ok(())
    }

    /// Scroll messages up
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    /// Scroll messages down
    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    /// Scroll to the bottom of messages
    pub fn scroll_to_bottom(&mut self) {
        // This will be clamped during rendering
        self.scroll_offset = self.messages.len().saturating_sub(1);
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
