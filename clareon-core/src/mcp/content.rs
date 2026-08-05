// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Flatten MCP content blocks into plain text for Clareon's tool/message model.

use rmcp::model::{ContentBlock, PromptMessage, ResourceContents, Role};

/// Flatten MCP content blocks into a single text string for the LLM.
pub fn flatten_content_blocks(blocks: &[ContentBlock]) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text(t) => parts.push(t.text.clone()),
            ContentBlock::Image(img) => {
                parts.push(format!("[image content omitted, mime={}]", img.mime_type));
            }
            ContentBlock::Audio(audio) => {
                parts.push(format!("[audio content omitted, mime={}]", audio.mime_type));
            }
            ContentBlock::Resource(embedded) => {
                parts.push(flatten_resource_contents(std::slice::from_ref(
                    &embedded.resource,
                )));
            }
            ContentBlock::ResourceLink(link) => {
                parts.push(format!("[resource link: {} ({})]", link.name, link.uri));
            }
            // Non-exhaustive: future variants
            _ => parts.push("[unsupported content block]".to_string()),
        }
    }
    parts.join("\n")
}

/// Flatten resource read contents to text.
pub fn flatten_resource_contents(contents: &[ResourceContents]) -> String {
    let mut parts = Vec::new();
    for c in contents {
        match c {
            ResourceContents::TextResourceContents {
                uri,
                text,
                mime_type,
                ..
            } => {
                let mime = mime_type.as_deref().unwrap_or("text/plain");
                parts.push(format!("--- {uri} ({mime}) ---\n{text}"));
            }
            ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
                ..
            } => {
                let mime = mime_type.as_deref().unwrap_or("application/octet-stream");
                let len = blob.len();
                parts.push(format!(
                    "--- {uri} ({mime}) ---\n[binary blob, {len} base64 chars omitted]"
                ));
            }
            _ => parts.push("[unknown resource content]".to_string()),
        }
    }
    if parts.is_empty() {
        "(empty resource)".to_string()
    } else {
        parts.join("\n\n")
    }
}

/// Convert an MCP prompt role to a Clareon role label ("user" / "assistant").
pub fn prompt_role_label(role: &Role) -> &'static str {
    match role {
        Role::Assistant => "assistant",
        Role::User => "user",
    }
}

/// Flatten prompt messages to a single text block for injection / display.
pub fn flatten_prompt_messages(messages: &[PromptMessage]) -> String {
    let mut parts = Vec::new();
    for msg in messages {
        let role = prompt_role_label(&msg.role);
        let body = flatten_content_blocks(std::slice::from_ref(&msg.content));
        parts.push(format!("[{role}]\n{body}"));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::ContentBlock;

    #[test]
    fn flattens_text() {
        let blocks = vec![ContentBlock::text("hello"), ContentBlock::text("world")];
        assert_eq!(flatten_content_blocks(&blocks), "hello\nworld");
    }
}
