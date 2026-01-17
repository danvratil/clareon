// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::fs;

use crate::tools::{ExecutionContext, Tool, ToolError, ToolResult};

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List contents of a directory. Returns file names, sizes, and types."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the directory to list"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        input: &Value,
        context: &ExecutionContext,
    ) -> Result<ToolResult, ToolError> {
        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path' field".to_string()))?;

        let path = PathBuf::from(path_str);

        if !path.is_absolute() {
            return Ok(ToolResult::error("Path must be absolute"));
        }

        // Check if path is in allowed sandbox directories
        let host_path = if path.starts_with(context.sandbox.workspace()) {
            // Map /home/claude/* to host workspace path
            context.workspace.workspace().join(
                path.strip_prefix(context.sandbox.workspace())
                    .map_err(|e| ToolError::InvalidInput(e.to_string()))?,
            )
        } else if path.starts_with(context.sandbox.input_dir()) {
            // Map /mnt/user-data/uploads/* to host input path
            context.workspace.input().join(
                path.strip_prefix(context.sandbox.input_dir())
                    .map_err(|e| ToolError::InvalidInput(e.to_string()))?,
            )
        } else if path.starts_with(context.sandbox.output_dir()) {
            // Map /mnt/user-data/outputs/* to host output path
            context.workspace.output().join(
                path.strip_prefix(context.sandbox.output_dir())
                    .map_err(|e| ToolError::InvalidInput(e.to_string()))?,
            )
        } else {
            return Ok(ToolResult::error(format!(
                "Access denied: {} is not in workspace or workspace input or output directory",
                path.display()
            )));
        };

        // Read directory using host path
        let mut entries = match fs::read_dir(&host_path).await {
            Ok(e) => e,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Failed to read directory: {}",
                    e
                )));
            }
        };

        let mut items = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let metadata = entry.metadata().await.ok();
            let name = entry.file_name().to_string_lossy().to_string();

            let item = if let Some(meta) = metadata {
                if meta.is_dir() {
                    format!("{}/", name)
                } else {
                    format!("{} ({} bytes)", name, meta.len())
                }
            } else {
                name
            };

            items.push(item);
        }

        // Sort for consistent output
        items.sort();

        if items.is_empty() {
            Ok(ToolResult::success("Directory is empty"))
        } else {
            Ok(ToolResult::success(format!(
                "Contents of {}:\n{}",
                path.display(),
                items.join("\n")
            )))
        }
    }

    fn requires_sandbox(&self) -> bool {
        false // Read-only operation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{NoneSandbox, PersistentWorkspace};
    use crate::types::ConversationId;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_context() -> (ExecutionContext, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache_root = temp_dir.path();
        let shared_pip = cache_root.join("shared").join("pip");

        let workspace =
            PersistentWorkspace::new("test-conversation", cache_root, &shared_pip).unwrap();
        workspace.ensure_directories().await.unwrap();

        let context = ExecutionContext {
            conversation_id: ConversationId::from("test-conversation"),
            workspace: Arc::new(workspace),
            sandbox: Arc::new(NoneSandbox),
            env_vars: HashMap::new(),
        };

        (context, temp_dir)
    }

    #[tokio::test]
    async fn test_list_workspace_directory() {
        let (context, _temp_dir) = create_test_context().await;

        // Create test files in workspace
        let test_file = context.workspace.workspace().join("test.txt");
        fs::write(&test_file, b"test content").await.unwrap();

        let test_dir = context.workspace.workspace().join("subdir");
        fs::create_dir(&test_dir).await.unwrap();

        // List using sandbox path
        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/home/claude"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.txt"));
        assert!(result.output.contains("subdir/"));
    }

    #[tokio::test]
    async fn test_list_input_directory() {
        let (context, _temp_dir) = create_test_context().await;

        // Create test file in input directory
        let test_file = context.workspace.input().join("input.txt");
        fs::write(&test_file, b"input data").await.unwrap();

        // List using sandbox path
        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/mnt/user-data/uploads"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("input.txt"));
    }

    #[tokio::test]
    async fn test_list_output_directory() {
        let (context, _temp_dir) = create_test_context().await;

        // Create test file in output directory
        let test_file = context.workspace.output().join("output.txt");
        fs::write(&test_file, b"output data").await.unwrap();

        // List using sandbox path
        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/mnt/user-data/outputs"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("output.txt"));
    }

    #[tokio::test]
    async fn test_list_subdirectory() {
        let (context, _temp_dir) = create_test_context().await;

        // Create subdirectory with files
        let subdir = context.workspace.workspace().join("mydir");
        fs::create_dir(&subdir).await.unwrap();
        fs::write(subdir.join("file1.txt"), b"content1")
            .await
            .unwrap();
        fs::write(subdir.join("file2.txt"), b"content2")
            .await
            .unwrap();

        // List using sandbox path
        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/home/claude/mydir"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("file1.txt"));
        assert!(result.output.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_reject_non_absolute_path() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = ListDirectoryTool;
        let input = json!({
            "path": "relative/path"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Path must be absolute"));
    }

    #[tokio::test]
    async fn test_reject_unauthorized_path() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/etc/passwd"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Access denied"));
    }

    #[tokio::test]
    async fn test_empty_directory() {
        let (context, _temp_dir) = create_test_context().await;

        // Workspace is empty by default
        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/home/claude"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Directory is empty"));
    }

    #[tokio::test]
    async fn test_nonexistent_directory() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = ListDirectoryTool;
        let input = json!({
            "path": "/home/claude/nonexistent"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Failed to read directory"));
    }
}
