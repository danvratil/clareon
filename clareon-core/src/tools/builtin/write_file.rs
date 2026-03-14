// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::tools::{ExecutionContext, Tool, ToolError, ToolResult};

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
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

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'content' field".to_string()))?;

        let path = PathBuf::from(path_str);

        // Validate path is absolute
        if !path.is_absolute() {
            return Ok(ToolResult::error("Path must be absolute"));
        }

        // Check if path is in workspace output directory
        if !path.starts_with(context.sandbox.output_dir())
            && !path.starts_with(context.sandbox.workspace())
        {
            return Ok(ToolResult::error(format!(
                "Access denied: {} is not in workspace or workspace output directory",
                path.display()
            )));
        }

        // Use sh -c with cat and stdin redirection
        match context
            .sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("cat > {}", path.display()),
                ],
                context,
                Some(content.as_bytes()),
                self.timeout(),
            )
            .await
        {
            Ok(result) => match result.exit_code {
                0 => Ok(ToolResult::success(format!(
                    "Successfully wrote {} bytes to {}",
                    content.len(),
                    path.display()
                ))),
                code => Ok(ToolResult::error(format!(
                    "Failed to write file, exit code {}: {}",
                    code,
                    String::from_utf8_lossy(&result.stderr)
                ))),
            },
            Err(e) => Ok(ToolResult::error(format!("Failed to write file: {}", e))),
        }
    }

    fn requires_sandbox(&self) -> bool {
        true // Write operations should be sandboxed
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
    async fn test_write_workspace_file() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/test.txt",
            "content": "Hello, workspace!"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Successfully wrote"));
        assert!(result.output.contains("17 bytes"));

        // Verify file was created with exact content
        let test_file = context.workspace.workspace().join("test.txt");
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "Hello, workspace!");
    }

    #[tokio::test]
    async fn test_write_output_file() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "path": "/mnt/user-data/outputs/result.txt",
            "content": "Output data"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);

        // Verify file was created in output directory with exact content
        let output_file = context.workspace.output().join("result.txt");
        let content = fs::read_to_string(&output_file).await.unwrap();
        assert_eq!(content, "Output data");
    }

    #[tokio::test]
    async fn test_write_subdirectory_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create subdirectory first
        let subdir = context.workspace.workspace().join("subdir");
        fs::create_dir(&subdir).await.unwrap();

        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/subdir/nested.txt",
            "content": "Nested content"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);

        // Verify file was created with exact content
        let content = fs::read_to_string(subdir.join("nested.txt")).await.unwrap();
        assert_eq!(content, "Nested content");
    }

    #[tokio::test]
    async fn test_write_multiline_file() {
        let (context, _temp_dir) = create_test_context().await;

        let content = "Line 1\nLine 2\nLine 3\n";
        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/multiline.txt",
            "content": content
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);

        // Verify content is written exactly as provided
        let test_file = context.workspace.workspace().join("multiline.txt");
        let read_content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(read_content, content);
    }

    #[tokio::test]
    async fn test_overwrite_existing_file() {
        let (context, _temp_dir) = create_test_context().await;

        // Create initial file
        let test_file = context.workspace.workspace().join("existing.txt");
        fs::write(&test_file, b"Original content").await.unwrap();

        // Overwrite it
        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/existing.txt",
            "content": "New content"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);

        // Verify content was overwritten with exact content
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "New content");
    }

    #[tokio::test]
    async fn test_write_empty_file() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/empty.txt",
            "content": ""
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("0 bytes"));

        // Verify file exists and is truly empty
        let test_file = context.workspace.workspace().join("empty.txt");
        assert!(test_file.exists());
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "");
    }

    #[tokio::test]
    async fn test_write_large_file() {
        let (context, _temp_dir) = create_test_context().await;

        let large_content = "X".repeat(50_000);
        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/large.txt",
            "content": large_content.clone()
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("50000 bytes"));

        // Verify content is written exactly
        let test_file = context.workspace.workspace().join("large.txt");
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, large_content);
    }

    #[tokio::test]
    async fn test_write_special_characters() {
        let (context, _temp_dir) = create_test_context().await;

        let special_content = "Special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?`~\nTab:\tNewline:\n";
        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/special.txt",
            "content": special_content
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(result.success);

        // Verify content is written exactly as provided
        let test_file = context.workspace.workspace().join("special.txt");
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, special_content);
    }

    #[tokio::test]
    async fn test_reject_relative_path() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "path": "relative/path.txt",
            "content": "test"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Path must be absolute"));
    }

    #[tokio::test]
    async fn test_reject_unauthorized_path() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "path": "/etc/passwd",
            "content": "malicious"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Access denied"));
    }

    #[tokio::test]
    async fn test_reject_write_to_input_directory() {
        let (context, _temp_dir) = create_test_context().await;

        // Input directory is read-only
        let tool = WriteFileTool;
        let input = json!({
            "path": "/mnt/user-data/uploads/test.txt",
            "content": "Should not work"
        });

        let result = tool.execute(&input, &context).await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Access denied"));
    }

    #[tokio::test]
    async fn test_missing_content_field() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "path": "/home/clareon/test.txt"
            // Missing "content" field
        });

        let result = tool.execute(&input, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_path_field() {
        let (context, _temp_dir) = create_test_context().await;

        let tool = WriteFileTool;
        let input = json!({
            "content": "test content"
            // Missing "path" field
        });

        let result = tool.execute(&input, &context).await;
        assert!(result.is_err());
    }
}
