// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use async_trait::async_trait;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

use crate::tools::{ExecutionContext, Sandbox, SandboxResult, ToolError};

const BWRAP_EXE: &str = "bwrap";

/// Blocked paths that should never be accessible
const BLOCKED_PATHS: &[&str] = &[
    "~/.ssh",
    "~/.gnupg",
    "~/.aws",
    "~/.config/gcloud",
    "~/.kube",
    "~/.docker",
    "/etc/shadow",
    "/etc/sudoers",
    "/proc/sys",
];

#[derive(Debug, Clone, Copy)]
pub enum SandboxMode {
    /// No network, strict isolation
    Strict,
    /// Basic isolation, no network
    Basic,
    /// Minimal isolation (for development)
    Minimal,
}

#[derive(Debug)]
pub struct BubblewrapSandbox {
    /// Sandbox mode configuration
    mode: SandboxMode,
}

impl BubblewrapSandbox {
    pub fn new(mode: SandboxMode) -> Self {
        Self { mode }
    }

    /// Check if bubblewrap binary exists
    pub fn check_available() -> bool {
        which::which(BWRAP_EXE).is_ok()
    }

    /// Construct bubblewrap command line
    fn build_command(&self, command: &[String], context: &ExecutionContext) -> Vec<String> {
        let mut args = vec![BWRAP_EXE.to_string()];

        // Basic isolation flags
        args.extend_from_slice(&[
            // Use new namespaces (user namespace automatic if unprivileged)
            "--unshare-pid".to_string(),
            "--unshare-ipc".to_string(),
            "--unshare-uts".to_string(),
            "--die-with-parent".to_string(),
        ]);

        // Network isolation (based on mode)
        if matches!(self.mode, SandboxMode::Strict | SandboxMode::Basic) {
            args.push("--unshare-net".to_string());
        }

        // Mount root as read-only
        args.extend_from_slice(&["--ro-bind".to_string(), "/".to_string(), "/".to_string()]);

        // Create writable /tmp
        args.extend_from_slice(&["--tmpfs".to_string(), "/tmp".to_string()]);

        // Make /home writable (tmpfs) so we can create /home/claude
        args.extend_from_slice(&["--tmpfs".to_string(), "/home".to_string()]);

        // Mount workspace to /home/claude (RW)
        args.extend_from_slice(&[
            "--bind".to_string(),
            context.workspace.workspace().to_string_lossy().to_string(),
            "/home/claude".to_string(),
        ]);

        // Mount shared pip cache to /home/claude/.local (RW)
        // Need to create the directory first
        args.extend_from_slice(&["--dir".to_string(), "/home/claude/.local".to_string()]);
        args.extend_from_slice(&[
            "--bind".to_string(),
            context.workspace.pip_cache().to_string_lossy().to_string(),
            "/home/claude/.local".to_string(),
        ]);

        // Make /mnt writable (tmpfs) so we can create mount points
        args.extend_from_slice(&["--tmpfs".to_string(), "/mnt".to_string()]);
        args.extend_from_slice(&["--dir".to_string(), "/mnt/user-data".to_string()]);

        // Mount input directory to /mnt/user-data/uploads (RO)
        if context.workspace.input().exists() {
            args.extend_from_slice(&[
                "--ro-bind".to_string(),
                context.workspace.input().to_string_lossy().to_string(),
                "/mnt/user-data/uploads".to_string(),
            ]);
        } else {
            args.extend_from_slice(&["--dir".to_string(), "/mnt/user-data/uploads".to_string()]);
        }

        // Mount output directory to /mnt/user-data/outputs (RW)
        args.extend_from_slice(&[
            "--bind".to_string(),
            context.workspace.output().to_string_lossy().to_string(),
            "/mnt/user-data/outputs".to_string(),
        ]);

        // Block sensitive paths (make them inaccessible)
        for blocked in BLOCKED_PATHS {
            let expanded = shellexpand::tilde(blocked);
            let path = Path::new(expanded.as_ref());
            if path.exists() {
                if path.is_dir() {
                    // For directories, mount tmpfs
                    args.extend_from_slice(&[
                        "--tmpfs".to_string(),
                        path.to_string_lossy().to_string(),
                    ]);
                } else {
                    // For files, bind /dev/null
                    args.extend_from_slice(&[
                        "--ro-bind".to_string(),
                        "/dev/null".to_string(),
                        path.to_string_lossy().to_string(),
                    ]);
                }
            }
        }

        // Set working directory to /home/claude
        args.extend_from_slice(&["--chdir".to_string(), "/home/claude".to_string()]);

        // Clear environment and set only what's needed
        args.push("--clearenv".to_string());

        // Set essential env vars
        for (key, value) in &context.env_vars {
            args.extend_from_slice(&["--setenv".to_string(), key.clone(), value.clone()]);
        }

        // Add HOME, PATH, USER
        args.extend_from_slice(&[
            "--setenv".to_string(),
            "HOME".to_string(),
            "/home/claude".to_string(),
        ]);
        args.extend_from_slice(&[
            "--setenv".to_string(),
            "PATH".to_string(),
            "/usr/bin:/bin:/home/claude/.local/bin".to_string(),
        ]);
        args.extend_from_slice(&[
            "--setenv".to_string(),
            "USER".to_string(),
            "claude".to_string(),
        ]);

        // Add the actual command to execute
        args.push("--".to_string());
        args.extend(command.iter().cloned());

        args
    }
}

#[async_trait]
impl Sandbox for BubblewrapSandbox {
    fn name(&self) -> &str {
        "bubblewrap"
    }

    fn is_available(&self) -> bool {
        Self::check_available()
    }

    async fn execute(
        &self,
        command: &[String],
        context: &ExecutionContext,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<SandboxResult, ToolError> {
        if !self.is_available() {
            return Err(ToolError::SandboxNotAvailable(
                "bubblewrap not found at /usr/bin/bwrap".to_string(),
            ));
        }

        let args = self.build_command(command, context);
        debug!("Executing sandboxed command: {:?}", args);

        let mut child = Command::new(&args[0])
            .args(&args[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn bwrap: {}", e)))?;

        // Write stdin if provided
        if let Some(data) = stdin
            && let Some(mut child_stdin) = child.stdin.take()
        {
            child_stdin
                .write_all(data)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write stdin: {}", e)))?;
        }

        // Wait with timeout
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| ToolError::Timeout(timeout))?
            .map_err(|e| ToolError::ExecutionFailed(format!("Command failed: {}", e)))?;

        Ok(SandboxResult {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::workspace::PersistentWorkspace;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper function to create a test execution context
    async fn create_test_context() -> (ExecutionContext, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache_root = temp_dir.path().join("cache");
        let shared_pip = temp_dir.path().join("shared_pip");

        let workspace = PersistentWorkspace::new(1, &cache_root, &shared_pip).unwrap();
        workspace.ensure_directories().await.unwrap();

        // Create a NoneSandbox for path mapping in tests (not used for actual execution)
        let sandbox = super::super::NoneSandbox;

        let context = ExecutionContext {
            conversation_id: 1,
            workspace: Arc::new(workspace),
            sandbox: Arc::new(sandbox),
            env_vars: HashMap::new(),
        };

        (context, temp_dir)
    }

    /// Helper to assert bubblewrap is available, failing the test if not
    fn require_bubblewrap(sandbox: &BubblewrapSandbox) {
        assert!(
            sandbox.is_available(),
            "bubblewrap must be installed to run these tests. \
             Install with: sudo apt install bubblewrap (Ubuntu/Debian), \
             sudo dnf install bubblewrap (Fedora), \
             sudo pacman -S bubblewrap (Arch)"
        );
    }

    // ===== 1. AVAILABILITY TESTS =====

    #[test]
    fn test_bubblewrap_check_available() {
        // This test verifies that BubblewrapSandbox::check_available()
        // correctly detects if bwrap is installed
        let available = BubblewrapSandbox::check_available();
        assert!(available, "bubblewrap not found - install it to run tests");
    }

    #[tokio::test]
    async fn test_bubblewrap_is_available() {
        // Verify is_available() method works correctly
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        assert!(
            sandbox.is_available(),
            "bubblewrap not found - install it to run tests"
        );
    }

    #[tokio::test]
    async fn test_bubblewrap_version() {
        // Execute `bwrap --version` to ensure binary works
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &["bwrap".to_string(), "--version".to_string()],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute bwrap --version");

        assert_eq!(result.exit_code, 0);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("bubblewrap") || stdout.contains("bwrap"));
    }

    // ===== 2. BASIC EXECUTION TESTS =====

    #[tokio::test]
    async fn test_execute_simple_command() {
        // Execute `echo "hello"` in sandbox
        // Verify stdout contains "hello" and exit code is 0
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &["echo".to_string(), "hello".to_string()],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute echo");

        assert_eq!(result.exit_code, 0);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert_eq!(stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn test_execute_with_stdin() {
        // Execute `cat` with stdin data
        // Verify output matches input
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let input = b"test input data\n";
        let result = sandbox
            .execute(
                &["cat".to_string()],
                &context,
                Some(input),
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute cat");

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, input);
    }

    #[tokio::test]
    async fn test_execute_with_timeout() {
        // Execute `sleep 10` with 1 second timeout
        // Verify timeout error is returned
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &["sleep".to_string(), "10".to_string()],
                &context,
                None,
                Duration::from_secs(1),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::Timeout(duration) => {
                assert_eq!(duration, Duration::from_secs(1));
            }
            other => panic!("Expected Timeout error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_nonzero_exit() {
        // Execute `sh -c "exit 42"`
        // Verify exit code is captured correctly
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &["sh".to_string(), "-c".to_string(), "exit 42".to_string()],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute sh");

        assert_eq!(result.exit_code, 42);
    }

    // ===== 3. ISOLATION VERIFICATION TESTS =====

    #[tokio::test]
    async fn test_network_isolation_strict() {
        // Strict mode: network should be disabled
        // Try to resolve a hostname - should fail
        let sandbox = BubblewrapSandbox::new(SandboxMode::Strict);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Try to use ping (which requires network)
        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "ping -c 1 -W 1 8.8.8.8 2>&1 || echo NETWORK_BLOCKED".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(10),
            )
            .await
            .expect("Failed to execute ping test");

        let output = String::from_utf8_lossy(&result.stdout);
        // Network should be blocked, so ping should fail
        assert!(
            output.contains("NETWORK_BLOCKED") || output.contains("Network is unreachable"),
            "Network should be blocked in Strict mode, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_filesystem_read_only() {
        // Try to create a file in /usr (should fail - read-only)
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "touch /usr/test-file 2>&1 || echo READONLY".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute touch");

        let output = String::from_utf8_lossy(&result.stdout);
        assert!(
            output.contains("READONLY") || output.contains("Read-only"),
            "Root filesystem should be read-only, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_blocked_sensitive_paths_ssh() {
        // Test that ~/.ssh is blocked/inaccessible
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Try to list ~/.ssh directory
        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "ls ~/.ssh 2>&1 || echo BLOCKED".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute ls");

        let output = String::from_utf8_lossy(&result.stdout);
        // Should either be blocked or show an empty tmpfs mount
        assert!(
            output.contains("BLOCKED")
                || output.contains("No such file")
                || output.trim().is_empty(),
            "~/.ssh should be blocked, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_workspace_access() {
        // Create file in workspace, verify it's readable in sandbox
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Write test file to workspace
        let test_file = context.workspace.workspace().join("test.txt");
        fs::write(&test_file, "workspace content").unwrap();

        // Read it from sandbox
        let result = sandbox
            .execute(
                &["cat".to_string(), "/home/claude/test.txt".to_string()],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute cat");

        assert_eq!(result.exit_code, 0);
        let output = String::from_utf8_lossy(&result.stdout);
        assert_eq!(output, "workspace content");
    }

    #[tokio::test]
    async fn test_workspace_writable() {
        // Execute command to write to workspace, verify file is created
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Write file from sandbox
        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'test output' > /home/claude/output.txt".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute sh");

        assert_eq!(result.exit_code, 0);

        // Verify file exists on host
        let output_file = context.workspace.workspace().join("output.txt");
        assert!(output_file.exists());
        let content = fs::read_to_string(&output_file).unwrap();
        assert_eq!(content.trim(), "test output");
    }

    #[tokio::test]
    async fn test_tmp_writable() {
        // Execute command to write to /tmp (tmpfs), should succeed
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'test' > /tmp/test.txt && cat /tmp/test.txt".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute sh");

        assert_eq!(result.exit_code, 0);
        let output = String::from_utf8_lossy(&result.stdout);
        assert_eq!(output.trim(), "test");
    }

    // ===== 4. PATH MAPPING TESTS =====

    #[tokio::test]
    async fn test_input_directory_readonly() {
        // Create file in input directory, verify it's readable but not writable
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Write test file to input directory
        let test_file = context.workspace.input().join("input.txt");
        fs::write(&test_file, "input content").unwrap();

        // 1. Verify it's readable
        let result = sandbox
            .execute(
                &[
                    "cat".to_string(),
                    "/mnt/user-data/uploads/input.txt".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute cat");

        assert_eq!(result.exit_code, 0);
        let output = String::from_utf8_lossy(&result.stdout);
        assert_eq!(output, "input content");

        // 2. Verify it's NOT writable
        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'fail' > /mnt/user-data/uploads/input.txt 2>&1 || echo READONLY"
                        .to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute write test");

        let output = String::from_utf8_lossy(&result.stdout);
        assert!(
            output.contains("READONLY") || output.contains("Read-only"),
            "Input directory should be read-only, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_output_directory_writable() {
        // Execute command to write to output directory
        // Verify file appears in host output directory
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Write file to output directory from sandbox
        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'output content' > /mnt/user-data/outputs/result.txt".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute sh");

        assert_eq!(result.exit_code, 0);

        // Verify file exists on host
        let output_file = context.workspace.output().join("result.txt");
        assert!(output_file.exists());
        let content = fs::read_to_string(&output_file).unwrap();
        assert_eq!(content.trim(), "output content");
    }

    // ===== 5. ENVIRONMENT VARIABLE TESTS =====

    #[tokio::test]
    async fn test_environment_cleared() {
        // Execute `env` in sandbox
        // Verify only expected vars present (HOME, PATH, USER)
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(&["env".to_string()], &context, None, Duration::from_secs(5))
            .await
            .expect("Failed to execute env");

        assert_eq!(result.exit_code, 0);
        let output = String::from_utf8_lossy(&result.stdout);

        // Should have HOME, PATH, USER
        assert!(output.contains("HOME=/home/claude"));
        assert!(output.contains("PATH="));
        assert!(output.contains("USER=claude"));

        // Should NOT have many host environment variables
        // (this is a basic check - the env should be minimal)
        let line_count = output.lines().count();
        assert!(
            line_count <= 10,
            "Environment should be minimal, found {} variables:\n{}",
            line_count,
            output
        );
    }

    #[tokio::test]
    async fn test_custom_env_vars() {
        // Execute with custom env: FOO=bar
        // Verify env var is set
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (mut context, _temp_dir) = create_test_context().await;
        context
            .env_vars
            .insert("FOO".to_string(), "bar".to_string());
        context
            .env_vars
            .insert("TEST_VAR".to_string(), "test_value".to_string());

        let result = sandbox
            .execute(
                &[
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo FOO=$FOO TEST_VAR=$TEST_VAR".to_string(),
                ],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to execute sh");

        assert_eq!(result.exit_code, 0);
        let output = String::from_utf8_lossy(&result.stdout);
        assert!(output.contains("FOO=bar"));
        assert!(output.contains("TEST_VAR=test_value"));
    }

    // ===== 6. SANDBOX MODE TESTS =====

    #[tokio::test]
    async fn test_strict_mode_network_disabled() {
        // Strict mode should have network disabled
        let sandbox = BubblewrapSandbox::new(SandboxMode::Strict);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Verify --unshare-net is in the command
        let command = sandbox.build_command(&["echo".to_string()], &context);
        assert!(
            command.contains(&"--unshare-net".to_string()),
            "Strict mode should include --unshare-net"
        );
    }

    #[tokio::test]
    async fn test_basic_mode_network_disabled() {
        // Basic mode should have network disabled
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Verify --unshare-net is in the command
        let command = sandbox.build_command(&["echo".to_string()], &context);
        assert!(
            command.contains(&"--unshare-net".to_string()),
            "Basic mode should include --unshare-net"
        );
    }

    #[tokio::test]
    async fn test_minimal_mode_network_enabled() {
        // Minimal mode should NOT have --unshare-net
        let sandbox = BubblewrapSandbox::new(SandboxMode::Minimal);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        // Verify --unshare-net is NOT in the command
        let command = sandbox.build_command(&["echo".to_string()], &context);
        assert!(
            !command.contains(&"--unshare-net".to_string()),
            "Minimal mode should NOT include --unshare-net"
        );
    }

    // ===== 7. ERROR HANDLING TESTS =====

    #[tokio::test]
    async fn test_command_not_found() {
        // Execute non-existent command
        // Verify error is properly reported
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(
                &["nonexistent_command_xyz".to_string()],
                &context,
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Command execution should complete");

        // Exit code should be non-zero (command not found)
        assert_ne!(result.exit_code, 0);

        // stderr should contain error message
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("not found") || stderr.contains("No such file"),
            "Should report command not found, got: {}",
            stderr
        );
    }

    #[tokio::test]
    async fn test_working_directory() {
        // Verify working directory is set to /home/claude
        let sandbox = BubblewrapSandbox::new(SandboxMode::Basic);
        require_bubblewrap(&sandbox);

        let (context, _temp_dir) = create_test_context().await;

        let result = sandbox
            .execute(&["pwd".to_string()], &context, None, Duration::from_secs(5))
            .await
            .expect("Failed to execute pwd");

        assert_eq!(result.exit_code, 0);
        let output = String::from_utf8_lossy(&result.stdout);
        assert_eq!(output.trim(), "/home/claude");
    }
}
