// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

mod approval;
mod artifacts;
mod builtin;
mod executor;
mod registry;
mod sandbox;
mod traits;
mod workspace;

pub use approval::{AlwaysAllowRule, is_denied, tools_need_approval};
pub use artifacts::ArtifactManager;
pub use builtin::{ListDirectoryTool, ReadFileTool, WriteFileTool, register_builtin_tools};
pub use executor::ToolExecutor;
pub use registry::ToolRegistry;
pub use sandbox::{BubblewrapSandbox, NoneSandbox, SandboxMode};
pub use traits::{ExecutionContext, Sandbox, SandboxResult, Tool, ToolResult};
pub use workspace::{PersistentWorkspace, WorkspaceManager};

use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error(
        "Sandbox not available: {0}\n\nTo install bubblewrap:\n  - Ubuntu/Debian: sudo apt install bubblewrap\n  - Fedora: sudo dnf install bubblewrap\n  - Arch: sudo pacman -S bubblewrap"
    )]
    SandboxNotAvailable(String),

    #[error("Workspace creation failed: {0}")]
    WorkspaceCreationFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
