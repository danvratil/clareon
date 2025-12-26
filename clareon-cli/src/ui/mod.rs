//! UI components for ratatui

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, ViewMode};
use clareon_core::types::Role;

/// Render the UI based on current app state
pub fn render(frame: &mut Frame, app: &App) {
    match app.view_mode {
        ViewMode::Chat => render_chat(frame, app),
        ViewMode::ConversationList => render_conversation_list(frame, app),
        ViewMode::SearchResults => render_search_results(frame, app),
        ViewMode::Help => {
            render_chat(frame, app);
            render_help_popup(frame);
        }
    }
}

/// Render the main chat view
fn render_chat(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(5),    // Messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    // Title bar
    let title = format!(
        " Clareon - {} | {} ",
        app.conversation_title(),
        app.manager.backend_name()
    );
    let title_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(title_block, chunks[0]);

    // Messages area
    render_messages(frame, app, chunks[1]);

    // Input area
    let input_block = Block::default()
        .title(" Message (Enter to send) ")
        .borders(Borders::ALL)
        .border_style(if app.waiting {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        });

    let input_text = if app.waiting {
        "Waiting for response..."
    } else {
        &app.input
    };

    let input = Paragraph::new(input_text)
        .block(input_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(input, chunks[2]);

    // Status bar
    let status_text = app
        .status
        .as_deref()
        .unwrap_or("Ctrl+H help | Ctrl+N new | Ctrl+O open | Ctrl+Q quit");
    let status = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, chunks[3]);
}

/// Render messages in the chat
fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.messages.is_empty() {
        let placeholder = Paragraph::new("Start a conversation by typing a message below...")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner_area);
        return;
    }

    // Build message items
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            let (prefix, style) = match msg.role {
                Role::User => ("You: ", Style::default().fg(Color::Cyan)),
                Role::Assistant => ("Claude: ", Style::default().fg(Color::Green)),
            };

            let text = msg.text().unwrap_or("[no text content]");
            let lines: Vec<Line> = text
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        Line::from(vec![
                            Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                            Span::raw(line),
                        ])
                    } else {
                        Line::from(format!("       {}", line))
                    }
                })
                .collect();

            ListItem::new(Text::from(lines))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner_area);
}

/// Render conversation list view
fn render_conversation_list(frame: &mut Frame, app: &App) {
    let block = Block::default()
        .title(" Conversations (Press number to select, Esc to cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    if app.conversations.is_empty() {
        let placeholder =
            Paragraph::new("No conversations yet").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner_area);
        return;
    }

    let items: Vec<ListItem> = app
        .conversations
        .iter()
        .enumerate()
        .take(9) // Only show 9 items (1-9 selection)
        .map(|(i, conv)| {
            let title = conv.display_title();
            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", i + 1),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(title),
                Span::styled(
                    format!(" ({} messages)", conv.message_count),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner_area);
}

/// Render search results view
fn render_search_results(frame: &mut Frame, app: &App) {
    let title = format!(
        " Search results for '{}' (Press number to select, Esc to cancel) ",
        app.search_query
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    if app.search_results.is_empty() {
        let placeholder =
            Paragraph::new("No results found").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner_area);
        return;
    }

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .take(9)
        .map(|(i, result)| {
            let title = result.conversation_title.as_deref().unwrap_or("Untitled");
            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", i + 1),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![Span::raw("    "), Span::raw(&result.snippet)]),
            ];
            ListItem::new(Text::from(lines))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner_area);
}

/// Render help popup
fn render_help_popup(frame: &mut Frame) {
    let area = centered_rect(60, 50, frame.area());

    frame.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            "Clareon Help",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Keyboard shortcuts:"),
        Line::from(""),
        Line::from("  Enter      Send message"),
        Line::from("  Ctrl+N     New conversation"),
        Line::from("  Ctrl+O     Open conversation list"),
        Line::from("  Ctrl+Q     Quit"),
        Line::from("  Ctrl+C     Quit"),
        Line::from("  Ctrl+U     Clear input"),
        Line::from("  Ctrl+W     Delete word"),
        Line::from("  Up/Down    Scroll messages"),
        Line::from("  Ctrl+H/F1  Show this help"),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc or Enter to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(Text::from(help_text))
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(help, area);
}

/// Helper to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
