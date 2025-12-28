// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use async_trait::async_trait;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::tools::{ExecutionContext, Sandbox, SandboxResult, ToolError};

/// No-op sandbox that executes commands directly (for testing only)
/// Maps sandbox paths to host paths to simulate the real sandbox behavior
#[derive(Debug)]
pub struct NoneSandbox;

impl NoneSandbox {
    /// Map sandbox paths in a command string to host paths
    fn map_paths_in_string(&self, s: &str, context: &ExecutionContext) -> String {
        let mut result = s.to_string();

        // Replace sandbox paths with host paths (longest paths first to avoid conflicts)
        let replacements = vec![
            ("/mnt/user-data/uploads", context.workspace.input().to_string_lossy().to_string()),
            ("/mnt/user-data/outputs", context.workspace.output().to_string_lossy().to_string()),
            ("/home/claude", context.workspace.workspace().to_string_lossy().to_string()),
        ];

        for (sandbox_path, host_path) in replacements {
            result = result.replace(sandbox_path, &host_path);
        }

        result
    }

    /// Map command arguments, replacing sandbox paths with host paths
    fn map_command(&self, command: &[String], context: &ExecutionContext) -> Vec<String> {
        command.iter()
            .map(|arg| self.map_paths_in_string(arg, context))
            .collect()
    }
}

#[async_trait]
impl Sandbox for NoneSandbox {
    fn name(&self) -> &str {
        "none"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        command: &[String],
        context: &ExecutionContext,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<SandboxResult, ToolError> {
        // Map paths in command
        let mapped_command = self.map_command(command, context);

        // Execute through shell, joining all command parts
        let shell_command = mapped_command.join(" ");

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&shell_command)
            .current_dir(context.workspace.workspace())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &context.env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if let Some(data) = stdin && let Some(mut child_stdin) = child.stdin.take() {
            child_stdin.write_all(data).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to write stdin: {}", e))
            })?;
        }

        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| ToolError::Timeout(timeout))?
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(SandboxResult {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}
