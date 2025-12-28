// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::tools::{ExecutionContext, Tool, ToolError, ToolResult};

pub struct ReadFileTool;

fn truncate_output(output: String) -> Result<ToolResult, ToolError> {
    const MAX_SIZE: usize = 100_000; // ~100KB
    if output.len() > MAX_SIZE {
        Ok(ToolResult::success(format!(
            "File too large ({} bytes). First {} bytes:\n\n{}",
            output.len(),
            MAX_SIZE,
            &output[..MAX_SIZE]
        )))
    } else {
        Ok(ToolResult::success(output))
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Returns the file contents as text."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
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
        // Parse input
        let path_str = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path' field".to_string()))?;

        let path = PathBuf::from(path_str);

        // Validate path is absolute
        if !path.is_absolute() {
            return Ok(ToolResult::error("Path must be absolute"));
        }

        // Check if path is in workspace input directory
        if !path.starts_with(context.sandbox.workspace())
            && !path.starts_with(context.sandbox.input_dir())
            && !path.starts_with(context.sandbox.output_dir())
        {
            return Ok(ToolResult::error(format!(
                "Access denied: {} is not in workspace or workspace input directory",
                path.display()
            )));
        }

        match context
            .sandbox
            .execute(
                &["cat".to_string(), path.display().to_string()],
                context,
                None,
                self.timeout(),
            )
            .await
        {
            Ok(output) => {
                if output.exit_code == 0 {
                    let output_str = String::from_utf8_lossy(&output.stdout).to_string();
                    return truncate_output(output_str);
                } else {
                    return Ok(ToolResult::error(format!(
                        "Failed to read file: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )));
                }
            }
            Err(e) => {
                return Ok(ToolResult::error(format!("Failed to read file: {}", e)));
            }
        }
    }

    fn requires_sandbox(&self) -> bool {
        false // Read-only operation, low risk
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
    use tokio::fs;

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
    async fn test_read_workspace_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create test file in workspace
        let test_file = context.workspace.workspace().join("test.txt");
        fs::write(&test_file, b"Hello, workspace!").await.unwrap();

        // Read using sandbox path
        let tool = ReadFileTool;
        let input = json!({
            "path": "/home/claude/test.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        if !result.success {
            eprintln!("Test failed with error: {}", result.output);
        }
        assert!(result.success, "Failed: {}", result.output);
        assert_eq!(result.output, "Hello, workspace!");
    }

    #[tokio::test]
    async fn test_read_input_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create test file in input directory
        let test_file = context.workspace.input().join("input.txt");
        fs::write(&test_file, b"Input data here").await.unwrap();

        // Read using sandbox path
        let tool = ReadFileTool;
        let input = json!({
            "path": "/mnt/user-data/uploads/input.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "Input data here");
    }

    #[tokio::test]
    async fn test_read_output_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create test file in output directory
        let test_file = context.workspace.output().join("output.txt");
        fs::write(&test_file, b"Output results").await.unwrap();

        // Read using sandbox path
        let tool = ReadFileTool;
        let input = json!({
            "path": "/mnt/user-data/outputs/output.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "Output results");
    }

    #[tokio::test]
    async fn test_read_subdirectory_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create file in subdirectory
        let subdir = context.workspace.workspace().join("subdir");
        fs::create_dir(&subdir).await.unwrap();
        fs::write(subdir.join("nested.txt"), b"Nested content")
            .await
            .unwrap();

        // Read using sandbox path
        let tool = ReadFileTool;
        let input = json!({
            "path": "/home/claude/subdir/nested.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "Nested content");
    }

    #[tokio::test]
    async fn test_read_multiline_file() {
        let (context, _temp_dir) = create_test_context().await;

        let content = "Line 1\nLine 2\nLine 3\n";
        let test_file = context.workspace.workspace().join("multiline.txt");
        fs::write(&test_file, content.as_bytes()).await.unwrap();

        let tool = ReadFileTool;
        let input = json!({
            "path": "/home/claude/multiline.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, content);
    }

    #[tokio::test]
    async fn test_read_large_file_truncation() {
        let (context, _temp_dir) = create_test_context().await;

        // Create a file larger than MAX_SIZE (100KB)
        let large_content = "X".repeat(150_000);
        let test_file = context.workspace.workspace().join("large.txt");
        fs::write(&test_file, large_content.as_bytes())
            .await
            .unwrap();

        let tool = ReadFileTool;
        let input = json!({
            "path": "/home/claude/large.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("File too large"));
        assert!(result.output.contains("150000 bytes"));
        assert!(result.output.contains("First 100000 bytes"));
    }

    #[tokio::test]
    async fn test_reject_relative_path() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = ReadFileTool;
        let input = json!({
            "path": "relative/path.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Path must be absolute"));
    }

    #[tokio::test]
    async fn test_reject_unauthorized_path() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = ReadFileTool;
        let input = json!({
            "path": "/etc/shadow"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Access denied"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = ReadFileTool;
        let input = json!({
            "path": "/home/claude/nonexistent.txt"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Failed to read file"));
    }

    #[tokio::test]
    async fn test_read_binary_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create a file with some binary data
        let binary_data = vec![0u8, 1, 2, 255, 254, 253];
        let test_file = context.workspace.workspace().join("binary.dat");
        fs::write(&test_file, &binary_data).await.unwrap();

        let tool = ReadFileTool;
        let input = json!({
            "path": "/home/claude/binary.dat"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        // Binary data should be converted to UTF-8 with replacement characters
        assert!(!result.output.is_empty());
    }
}
