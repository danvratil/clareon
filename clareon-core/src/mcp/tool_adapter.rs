// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adapter that exposes an MCP server tool as a Clareon [`Tool`].

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rmcp::model::CallToolRequestParams;
use rmcp::service::{Peer, RoleClient};
use serde_json::{Map, Value};
use tracing::{debug, warn};

use crate::tools::{ExecutionContext, Tool, ToolError, ToolResult};

use super::content::flatten_content_blocks;

/// Wraps a remote MCP tool so it can be registered in [`ToolRegistry`].
pub struct McpToolAdapter {
    /// Prefixed name seen by the LLM (`mcp_<server>_<tool>`).
    pub prefixed_name: String,
    /// Original tool name on the MCP server.
    pub remote_name: String,
    pub description: String,
    pub input_schema: Value,
    pub peer: Peer<RoleClient>,
    pub timeout: Duration,
}

impl McpToolAdapter {
    pub fn new(
        prefixed_name: impl Into<String>,
        remote_name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        peer: Peer<RoleClient>,
        timeout: Duration,
    ) -> Self {
        Self {
            prefixed_name: prefixed_name.into(),
            remote_name: remote_name.into(),
            description: description.into(),
            input_schema,
            peer,
            timeout,
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.prefixed_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn requires_sandbox(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: &Value,
        _context: &ExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let arguments: Option<Map<String, Value>> = match input {
            Value::Object(map) => Some(map.clone()),
            Value::Null => None,
            other => {
                // Coerce non-objects into a single-key map if possible; otherwise error.
                return Err(ToolError::InvalidInput(format!(
                    "MCP tool '{}' expects a JSON object argument, got {}",
                    self.remote_name, other
                )));
            }
        };

        let mut params = CallToolRequestParams::new(self.remote_name.clone());
        if let Some(args) = arguments {
            params = params.with_arguments(args);
        }

        debug!(
            "Calling MCP tool '{}' as '{}'",
            self.remote_name, self.prefixed_name
        );

        let result = tokio::time::timeout(self.timeout, self.peer.call_tool(params))
            .await
            .map_err(|_| ToolError::Timeout(self.timeout))?
            .map_err(|e| {
                warn!("MCP tool call failed: {e}");
                ToolError::ExecutionFailed(e.to_string())
            })?;

        let text = flatten_content_blocks(&result.content);
        let text = if text.is_empty() {
            if let Some(structured) = &result.structured_content {
                structured.to_string()
            } else {
                String::new()
            }
        } else {
            text
        };

        let is_error = result.is_error.unwrap_or(false);
        if is_error {
            Ok(ToolResult::error(if text.is_empty() {
                "MCP tool reported an error".to_string()
            } else {
                text
            }))
        } else {
            Ok(ToolResult::success(text))
        }
    }
}

/// Host meta-tool: list resources across connected MCP servers.
pub struct McpListResourcesTool {
    manager: Arc<super::McpManager>,
}

impl McpListResourcesTool {
    pub fn new(manager: Arc<super::McpManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for McpListResourcesTool {
    fn name(&self) -> &str {
        "mcp_list_resources"
    }

    fn description(&self) -> &str {
        "List resources available from connected MCP servers. Optionally filter by server id."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server": {
                    "type": "string",
                    "description": "Optional MCP server id to filter by"
                }
            }
        })
    }

    fn requires_sandbox(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: &Value,
        _context: &ExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let server_filter = input
            .get("server")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let resources = self.manager.list_resources(server_filter.as_deref()).await;
        let json = serde_json::to_string_pretty(&resources)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult::success(json))
    }
}

/// Host meta-tool: read a resource from an MCP server.
pub struct McpReadResourceTool {
    manager: Arc<super::McpManager>,
}

impl McpReadResourceTool {
    pub fn new(manager: Arc<super::McpManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for McpReadResourceTool {
    fn name(&self) -> &str {
        "mcp_read_resource"
    }

    fn description(&self) -> &str {
        "Read the contents of an MCP resource by server id and URI."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server": {
                    "type": "string",
                    "description": "MCP server id"
                },
                "uri": {
                    "type": "string",
                    "description": "Resource URI"
                }
            },
            "required": ["server", "uri"]
        })
    }

    fn requires_sandbox(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        input: &Value,
        _context: &ExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let server = input
            .get("server")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("missing 'server'".into()))?;
        let uri = input
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("missing 'uri'".into()))?;

        match self.manager.read_resource(server, uri).await {
            Ok(text) => Ok(ToolResult::success(text)),
            Err(e) => Ok(ToolResult::error(e)),
        }
    }
}
