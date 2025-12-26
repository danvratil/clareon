//! Clareon CLI - TUI client for Claude
//!
//! A terminal-based chat interface for interacting with Claude
//! via AWS Bedrock or the Anthropic API.

mod app;
mod cli;
mod events;
mod ui;

use std::fs::File;
use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use app::{App, AppOptions};
use clareon_core::Config;
use cli::Args;

/// Get the log file path
fn log_file_path() -> Result<PathBuf> {
    let data_dir = directories::ProjectDirs::from("org", "clareon", "clareon")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let log_dir = data_dir.data_dir();
    std::fs::create_dir_all(log_dir)?;
    Ok(log_dir.join("debug.log"))
}

/// Initialize file-based logging for TUI mode
fn init_file_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_path = log_file_path()?;

    // Open log file (truncate on each run for now)
    let log_file = File::create(&log_path)?;

    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .init();

    tracing::info!("Logging initialized to {:?}", log_path);

    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments first
    let args = Args::parse();

    // Initialize logging (only if not running TUI)
    if args.chats || args.search.is_some() {
        tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    // Load configuration
    let mut config = Config::load()?;

    // Override backend if specified
    if let Some(backend) = &args.backend {
        config.default_backend = backend.clone();
    }

    // Override model if specified
    if let Some(model) = &args.model {
        config.default_model = model.clone();
    }

    // Handle non-TUI modes
    if args.chats {
        return list_conversations(&config).await;
    }

    if let Some(query) = &args.search {
        return search_conversations(&config, query).await;
    }

    // Run TUI mode
    run_tui(config, args).await
}

/// List conversations and exit
async fn list_conversations(config: &Config) -> Result<()> {
    let db_url = Config::database_url()?;
    let storage = clareon_core::Storage::new(&db_url).await?;

    let conversations = storage.list_conversations().await?;

    if conversations.is_empty() {
        println!("No conversations found.");
        return Ok(());
    }

    println!("Conversations:\n");
    for conv in conversations {
        let title = conv.display_title();
        println!(
            "  {:>4}  {}  ({} messages)",
            conv.id, title, conv.message_count
        );
    }
    println!("\nUse --resume <ID> to continue a conversation.");

    Ok(())
}

/// Search conversations and exit
async fn search_conversations(config: &Config, query: &str) -> Result<()> {
    let db_url = Config::database_url()?;
    let storage = clareon_core::Storage::new(&db_url).await?;

    let results = storage.search(query).await?;

    if results.is_empty() {
        println!("No results found for '{}'", query);
        return Ok(());
    }

    println!("Search results for '{}':\n", query);
    for result in results {
        let title = result.conversation_title.as_deref().unwrap_or("Untitled");
        println!("  [{}] {} - {}", result.conversation_id, title, result.role);
        println!("       {}\n", result.snippet);
    }

    Ok(())
}

/// Run the TUI application
async fn run_tui(config: Config, args: Args) -> Result<()> {
    // Initialize file-based logging for TUI mode
    // The guard must be kept alive for the duration of the app
    let _log_guard = init_file_logging()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app with options
    let options = AppOptions {
        profile: args.profile.clone(),
        region: args.region.clone(),
    };
    let mut app = App::new(config, options).await?;

    // Resume conversation if specified
    if let Some(id) = args.resume {
        app.load_conversation(id).await?;
    }

    // Handle initial prompt if provided
    if let Some(prompt) = args.prompt {
        app.input = prompt;
        app.send_message().await?;
    }

    // Main loop
    let result = run_main_loop(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Main application loop
async fn run_main_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    while app.running {
        // Render
        terminal.draw(|f| ui::render(f, app))?;

        // Handle events
        events::handle_events(app).await?;
    }

    Ok(())
}
