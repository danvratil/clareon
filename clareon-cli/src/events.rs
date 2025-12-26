//! Event handling for the TUI

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ViewMode};

/// Handle events and update app state
pub async fn handle_events(app: &mut App) -> anyhow::Result<()> {
    // Poll for events with a timeout
    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) => handle_key_event(app, key).await?,
            Event::Resize(_, _) => {
                // Terminal resize - rendering will handle this
            }
            _ => {}
        }
    }

    Ok(())
}

/// Handle keyboard input
async fn handle_key_event(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    // Don't process input while waiting for response
    if app.waiting {
        return Ok(());
    }

    match app.view_mode {
        ViewMode::Chat => handle_chat_keys(app, key).await?,
        ViewMode::ConversationList => handle_list_keys(app, key).await?,
        ViewMode::SearchResults => handle_search_keys(app, key).await?,
        ViewMode::Help => handle_help_keys(app, key),
    }

    Ok(())
}

/// Handle keys in chat view
async fn handle_chat_keys(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match (key.code, key.modifiers) {
        // Quit
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
        | (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
            app.quit();
        }

        // Send message
        (KeyCode::Enter, KeyModifiers::NONE) => {
            if !app.input.is_empty() {
                app.send_message().await?;
            }
        }

        // New conversation
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.new_conversation().await?;
        }

        // Open conversation list
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
            app.load_conversation_list().await?;
        }

        // Show help (Ctrl+H or F1)
        (KeyCode::Char('h'), KeyModifiers::CONTROL) | (KeyCode::F(1), _) => {
            app.view_mode = ViewMode::Help;
        }

        // Scroll up
        (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::PageUp, _) => {
            app.scroll_up(3);
        }

        // Scroll down
        (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::PageDown, _) => {
            app.scroll_down(3);
        }

        // Character input
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            app.input.push(c);
        }

        // Backspace
        (KeyCode::Backspace, _) => {
            app.input.pop();
        }

        // Delete word
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            // Delete last word
            if let Some(pos) = app.input.trim_end().rfind(|c: char| c.is_whitespace()) {
                app.input.truncate(pos);
            } else {
                app.input.clear();
            }
        }

        // Clear input
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            app.input.clear();
        }

        _ => {}
    }

    Ok(())
}

/// Handle keys in conversation list view
async fn handle_list_keys(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        // Go back to chat
        KeyCode::Esc | KeyCode::Char('q') => {
            app.view_mode = ViewMode::Chat;
        }

        // Select conversation (for now, just use numbers)
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let index = c.to_digit(10).unwrap() as usize;
            if index > 0 && index <= app.conversations.len() {
                let conv_id = app.conversations[index - 1].id;
                app.load_conversation(conv_id).await?;
                app.view_mode = ViewMode::Chat;
            }
        }

        _ => {}
    }

    Ok(())
}

/// Handle keys in search results view
async fn handle_search_keys(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        // Go back
        KeyCode::Esc | KeyCode::Char('q') => {
            app.view_mode = ViewMode::Chat;
        }

        // Select result
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let index = c.to_digit(10).unwrap() as usize;
            if index > 0 && index <= app.search_results.len() {
                let conv_id = app.search_results[index - 1].conversation_id;
                app.load_conversation(conv_id).await?;
                app.view_mode = ViewMode::Chat;
            }
        }

        _ => {}
    }

    Ok(())
}

/// Handle keys in help view
fn handle_help_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
            app.view_mode = ViewMode::Chat;
        }
        _ => {}
    }
}
