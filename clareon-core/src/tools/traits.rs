// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, fmt::Debug};

use crate::backend::ToolDefinition;
use crate::types::ConversationId;

use super::{PersistentWorkspace, ToolError};

/// Trait for implementing tools that can be called by the LLM
#[async_trait]
pub trait Tool: Send + Sync {
    /// Name of the tool (must match what the LLM will call)
    fn name(&self) -> &str;

    /// Human-readable description for the LLM
    fn description(&self) -> &str;

    /// JSON Schema for input validation
    fn input_schema(&self) -> Value;

    /// Execute the tool with given input
    ///
    /// # Arguments
    /// * `input` - Validated JSON input matching the schema
    /// * `context` - Execution context (workspace path, allowed paths, etc.)
    ///
    /// # Returns
    /// JSON result or error
    async fn execute(
        &self,
        input: &Value,
        context: &ExecutionContext,
    ) -> Result<ToolResult, ToolError>;

    /// Maximum execution time (default: 30 seconds)
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// Whether this tool requires sandboxing (default: true)
    fn requires_sandbox(&self) -> bool {
        true
    }

    /// Generate ToolDefinition for LLM API
    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

/// Context provided to tool during execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Conversation ID
    pub conversation_id: ConversationId,

    /// Persistent workspace for this conversation
    pub workspace: Arc<PersistentWorkspace>,

    /// The sandbox in which the tool is being executed
    pub sandbox: Arc<dyn Sandbox>,

    /// Environment variables to set
    pub env_vars: HashMap<String, String>,
}

/// Result from tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Success or failure
    pub success: bool,

    /// Output text (what gets sent to LLM)
    pub output: String,

    /// Optional structured data (for future use)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            data: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: format!("Error: {}", message.into()),
            data: None,
        }
    }
}

/// Trait for sandbox implementations
#[async_trait]
pub trait Sandbox: Send + Sync + Debug {
    fn workspace(&self) -> PathBuf {
        PathBuf::from("/home/claude")
    }

    fn input_dir(&self) -> PathBuf {
        PathBuf::from("/mnt/user-data/uploads")
    }

    fn output_dir(&self) -> PathBuf {
        PathBuf::from("/mnt/user-data/outputs")
    }

    /// Name of the sandbox implementation
    fn name(&self) -> &str;

    /// Check if sandbox is available on the system
    fn is_available(&self) -> bool;

    /// Execute a command inside the sandbox
    ///
    /// # Arguments
    /// * `command` - Command and arguments to execute
    /// * `context` - Execution context (paths, env vars, etc.)
    /// * `stdin` - Optional stdin data
    /// * `timeout` - Maximum execution time
    ///
    /// # Returns
    /// Stdout, stderr, and exit code
    async fn execute(
        &self,
        command: &[String],
        context: &ExecutionContext,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<SandboxResult, ToolError>;
}

/// Result from sandbox execution
#[derive(Debug)]
pub struct SandboxResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}
