// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::types::{ContentBlock, ToolResultContent};

use super::{ArtifactManager, ExecutionContext, ToolRegistry, Sandbox, ToolResult, WorkspaceManager};

/// High-level tool executor that coordinates sandbox and tools
pub struct ToolExecutor {
    pub registry: Arc<ToolRegistry>,
    sandbox: Arc<dyn Sandbox>,
    workspace_manager: Arc<WorkspaceManager>,
    artifact_manager: Arc<ArtifactManager>,
}

impl ToolExecutor {
    pub fn new(
        registry: Arc<ToolRegistry>,
        sandbox: Arc<dyn Sandbox>,
        workspace_manager: Arc<WorkspaceManager>,
        artifact_manager: Arc<ArtifactManager>,
    ) -> Self {
        Self {
            registry,
            sandbox,
            workspace_manager,
            artifact_manager,
        }
    }

    /// Execute a tool use request
    ///
    /// Returns a ToolResult ContentBlock
    pub async fn execute_tool_use(
        &self,
        tool_use_id: &str,
        tool_name: &str,
        input: &Value,
        context: &ExecutionContext,
    ) -> ContentBlock {
        info!("Executing tool: {} (id: {})", tool_name, tool_use_id);
        debug!("Tool use {} input: {}", tool_use_id, input);

        // Look up tool
        let tool = match self.registry.get(tool_name) {
            Some(t) => t,
            None => {
                warn!("Tool not found: {}", tool_name);
                return ContentBlock::tool_result(
                    tool_use_id,
                    vec![ToolResultContent::text(format!(
                        "Error: Unknown tool '{}'",
                        tool_name
                    ))],
                    true, // is_error
                );
            }
        };

        // Execute tool
        let result = match tool.execute(input, context).await {
            Ok(r) => {
                if r.success {
                    info!("Tool {} (id: {}) executed successfully", tool_name, tool_use_id);
                    debug!("Tool use {} output: {}", tool_use_id, r.output);
                } else {
                    warn!("Tool {} (id: {}) execution failed: {}", tool_name, tool_use_id, r.output);
                }
                r
            },
            Err(e) => {
                warn!("Tool execution failed: {}", e);
                ToolResult::error(e.to_string())
            }
        };

        // Convert to ContentBlock
        ContentBlock::tool_result(
            tool_use_id,
            vec![ToolResultContent::text(result.output)],
            !result.success,
        )
    }

    /// Execute multiple tool uses in parallel with artifact synchronization
    pub async fn execute_multiple(
        &self,
        tool_uses: Vec<(&str, &str, &Value)>, // (id, name, input)
        conversation_id: i64,
        message_id: i64,
    ) -> Result<Vec<ContentBlock>, super::ToolError> {
        // 1. Get workspace
        let workspace = self.workspace_manager.get_workspace(conversation_id).await?;

        // 2. Scan output directory BEFORE execution (get baseline hashes)
        let before_hashes =
            ArtifactManager::scan_output_directory(workspace.output()).await?;

        // 3. Build execution context
        let context = ExecutionContext {
            conversation_id,
            workspace: workspace.clone(),
            sandbox: Arc::clone(self.sandbox()),
            env_vars: HashMap::new(),
        };

        // 4. Execute tools in parallel
        let mut tasks = Vec::new();

        for (id, name, input) in tool_uses {
            let executor = self.clone();
            let ctx = context.clone();
            let id = id.to_string();
            let name = name.to_string();
            let input = input.clone();

            tasks.push(tokio::spawn(async move {
                executor
                    .execute_tool_use(&id, &name, &input, &ctx)
                    .await
            }));
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => warn!("Tool execution task failed: {}", e),
            }
        }

        // 5. Scan and sync artifacts
        let synced_count = self
            .artifact_manager
            .sync_artifacts(conversation_id, message_id, &workspace, &before_hashes)
            .await?;

        if synced_count > 0 {
            info!("Synced {} new/updated artifacts to database", synced_count);
        }

        // 6. Update workspace metadata
        let disk_usage = workspace.calculate_disk_usage().await?;
        if let Err(e) = self
            .workspace_manager
            .storage
            .update_workspace_disk_usage(conversation_id, disk_usage as i64)
            .await
        {
            warn!("Failed to update workspace disk usage: {}", e);
        }

        Ok(results)
    }

    /// Get a reference to the sandbox
    pub fn sandbox(&self) -> &Arc<dyn Sandbox> {
        &self.sandbox
    }

    /// Get a reference to the workspace manager
    pub fn workspace_manager(&self) -> &WorkspaceManager {
        &self.workspace_manager
    }

    /// Get a reference to the artifact manager
    pub fn artifact_manager(&self) -> &ArtifactManager {
        &self.artifact_manager
    }
}

// Implement Clone for ToolExecutor to enable parallel execution
impl Clone for ToolExecutor {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            sandbox: self.sandbox.clone(),
            workspace_manager: self.workspace_manager.clone(),
            artifact_manager: self.artifact_manager.clone(),
        }
    }
}
