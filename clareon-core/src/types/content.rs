//! Content block types for messages

use serde::{Deserialize, Serialize};

/// A content block within a message.
///
/// Messages can contain multiple content blocks of different types,
/// such as text, tool use requests, and tool results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content
    Text {
        text: String,
    },

    /// A request from the assistant to use a tool
    ToolUse {
        /// Unique identifier for this tool use, used to match with results
        id: String,
        /// Name of the tool to invoke
        name: String,
        /// Input parameters for the tool (as JSON)
        input: serde_json::Value,
    },

    /// Result of a tool execution, sent back to the model
    ToolResult {
        /// ID matching the tool_use block this is responding to
        tool_use_id: String,
        /// The result content
        content: Vec<ToolResultContent>,
        /// Whether the tool execution resulted in an error
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    // Future variants:
    // Image { source: ImageSource, ... }
    // Document { source: DocumentSource, ... }
}

impl ContentBlock {
    /// Create a new text content block
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create a new tool use content block
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: serde_json::Value) -> Self {
        Self::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a new tool result content block
    pub fn tool_result(tool_use_id: impl Into<String>, content: Vec<ToolResultContent>, is_error: bool) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content,
            is_error: if is_error { Some(true) } else { None },
        }
    }

    /// Extract text content if this is a text block
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Check if this is a tool use block
    pub fn is_tool_use(&self) -> bool {
        matches!(self, Self::ToolUse { .. })
    }

    /// Check if this is a tool result block
    pub fn is_tool_result(&self) -> bool {
        matches!(self, Self::ToolResult { .. })
    }
}

/// Content within a tool result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContent {
    /// Text result from tool execution
    Text {
        text: String,
    },
    // Future variants:
    // Image { ... }
}

impl ToolResultContent {
    /// Create a new text tool result
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_block_serialization() {
        let text = ContentBlock::text("Hello, world!");
        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello, world!\""));

        let tool_use = ContentBlock::tool_use(
            "tool_123",
            "read_file",
            serde_json::json!({"path": "/tmp/test.txt"}),
        );
        let json = serde_json::to_string(&tool_use).unwrap();
        assert!(json.contains("\"type\":\"tool_use\""));
        assert!(json.contains("\"id\":\"tool_123\""));
    }

    #[test]
    fn test_content_block_deserialization() {
        let json = r#"{"type":"text","text":"Hello"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert_eq!(block.as_text(), Some("Hello"));
    }
}
