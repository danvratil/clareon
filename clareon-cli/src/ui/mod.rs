// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! UI components for ratatui

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, ViewMode};
use clareon_core::types::{ContentBlock, Role, ToolResultContent};

/// Helper to convert content blocks to displayable text
fn format_content_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.clone(),
            ContentBlock::ToolUse { id, name, input } => {
                format!(
                    "[Tool: {}]\nID: {}\nInput: {}",
                    name,
                    id,
                    serde_json::to_string_pretty(input).unwrap_or_else(|_| "{}".to_string())
                )
            }
            ContentBlock::ToolResult {
                tool_use_id: _,
                content,
                is_error,
            } => {
                let result_text = content
                    .iter()
                    .map(|c| match c {
                        ToolResultContent::Text { text } => text.as_str(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let status = if is_error.unwrap_or(false) {
                    "ERROR"
                } else {
                    "SUCCESS"
                };
                format!("[Tool Result: {}]\n{}", status, result_text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Wrap a line of text to fit within a maximum width
/// Returns multiple lines if wrapping is needed
fn wrap_line(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();

        // If adding this word would exceed the width
        if current_width + word_len + (if current_width > 0 { 1 } else { 0 }) > max_width {
            // If the word itself is longer than max_width, split it
            if word_len > max_width {
                // Finish current line if any
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                    current_width = 0;
                }

                // Split the long word across multiple lines
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max_width) {
                    lines.push(chunk.iter().collect());
                }
                continue;
            }

            // Start a new line with this word
            if !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line = word.to_string();
                current_width = word_len;
            } else {
                // This shouldn't happen, but just in case
                current_line = word.to_string();
                current_width = word_len;
            }
        } else {
            // Add word to current line
            if !current_line.is_empty() {
                current_line.push(' ');
                current_width += 1;
            }
            current_line.push_str(word);
            current_width += word_len;
        }
    }

    // Add the last line if any
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // If no lines were created, return at least one empty line
    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Render the UI based on current app state
pub fn render(frame: &mut Frame, app: &mut App) {
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
fn render_chat(frame: &mut Frame, app: &mut App) {
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
    let input_title = if app.waiting {
        " Streaming... (Esc to cancel) "
    } else {
        " Message (Enter to send) "
    };

    let input_block = Block::default()
        .title(input_title)
        .borders(Borders::ALL)
        .border_style(if app.waiting {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        });

    let input_text = if app.waiting {
        "Streaming response..."
    } else {
        &app.input
    };

    let input = Paragraph::new(input_text)
        .block(input_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(input, chunks[2]);

    // Status bar
    let status_text = if let Some(status) = &app.status {
        // If there's a status message (e.g., streaming, error), show it
        status.clone()
    } else {
        // Otherwise, show token info if available
        format_status_bar(app)
    };
    let status = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, chunks[3]);
}

/// Format the status bar with token information
fn format_status_bar(app: &App) -> String {
    let mut parts = Vec::new();

    // Last message tokens
    if let Some(usage) = &app.last_usage {
        // Build cache info string
        let mut cache_parts = Vec::new();
        if let Some(cached) = usage.cache_read_input_tokens
            && cached > 0
        {
            cache_parts.push(format!("⚡{}", format_number(cached)));
        }
        if let Some(written) = usage.cache_write_input_tokens
            && written > 0
        {
            cache_parts.push(format!("✍{}", format_number(written)));
        }

        let cache_info = if !cache_parts.is_empty() {
            format!(" ({})", cache_parts.join(" "))
        } else {
            String::new()
        };

        parts.push(format!(
            "Last: ↓{}{} ↑{}",
            format_number(usage.input_tokens),
            cache_info,
            format_number(usage.output_tokens)
        ));
    }

    // Conversation total
    if app.conversation_usage.input_tokens > 0 || app.conversation_usage.output_tokens > 0 {
        parts.push(format!(
            "Total: ↓{} ↑{}",
            format_number(app.conversation_usage.input_tokens),
            format_number(app.conversation_usage.output_tokens)
        ));
    }

    // Help text
    parts.push("Ctrl+H help | Ctrl+Q quit".to_string());

    parts.join(" | ")
}

/// Format a number with K/M suffix for readability
fn format_number(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Render messages in the chat
fn render_messages(frame: &mut Frame, app: &mut App, area: Rect) {
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

    // Calculate available width for text (account for prefix like "You: " and indentation)
    let prefix_width = 8; // "Claude: " or "You: " + 1 space
    let available_width = inner_area.width.saturating_sub(prefix_width) as usize;

    // Build message items
    let mut items: Vec<ListItem> = app
        .messages
        .iter()
        .flat_map(|msg| {
            let (prefix, style) = match msg.role {
                Role::User => ("You: ", Style::default().fg(Color::Cyan)),
                Role::Assistant => ("Claude: ", Style::default().fg(Color::Green)),
            };

            let text = format_content_blocks(&msg.content);
            let mut all_lines: Vec<Line> = Vec::new();
            let mut is_first_line = true;

            for original_line in text.lines() {
                // Wrap each line to fit the available width
                let wrapped_lines = wrap_line(original_line, available_width.max(20));

                for wrapped in wrapped_lines {
                    if is_first_line {
                        all_lines.push(Line::from(vec![
                            Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                            Span::raw(wrapped),
                        ]));
                        is_first_line = false;
                    } else {
                        all_lines.push(Line::from(format!("       {}", wrapped)));
                    }
                }
            }

            // Return as a single ListItem per message
            vec![ListItem::new(Text::from(all_lines))]
        })
        .collect();

    // Append streaming message if present
    if let Some(partial) = &app.streaming_message {
        let text = format_content_blocks(&partial.content);

        if !text.is_empty() {
            let mut all_lines: Vec<Line> = Vec::new();
            let mut is_first_line = true;

            for original_line in text.lines() {
                // Wrap each line to fit the available width
                let wrapped_lines = wrap_line(original_line, available_width.max(20));

                for wrapped in wrapped_lines {
                    if is_first_line {
                        all_lines.push(Line::from(vec![
                            Span::styled(
                                "Claude: ",
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(wrapped),
                        ]));
                        is_first_line = false;
                    } else {
                        all_lines.push(Line::from(format!("       {}", wrapped)));
                    }
                }
            }

            // Add blinking cursor to last line
            if let Some(last_line) = all_lines.last_mut() {
                last_line.spans.push(Span::styled(
                    " ▊",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::SLOW_BLINK),
                ));
            }

            items.push(ListItem::new(Text::from(all_lines)));
        } else {
            // Show "thinking" indicator if no content yet
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    "Claude: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "thinking...",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ])));
        }
    }

    let list = List::new(items);
    frame.render_stateful_widget(list, inner_area, &mut app.message_list_state);
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
