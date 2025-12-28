// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

mod traits;
mod registry;
mod executor;
mod sandbox;
mod builtin;
mod workspace;
mod artifacts;

pub use traits::{
    Tool, Sandbox, ExecutionContext, ToolResult, SandboxResult,
};
pub use registry::ToolRegistry;
pub use executor::ToolExecutor;
pub use sandbox::{BubblewrapSandbox, NoneSandbox, SandboxMode};
pub use builtin::{register_builtin_tools, ReadFileTool, WriteFileTool, ListDirectoryTool};
pub use workspace::{PersistentWorkspace, WorkspaceManager};
pub use artifacts::ArtifactManager;

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

    #[error("Sandbox not available: {0}\n\nTo install bubblewrap:\n  - Ubuntu/Debian: sudo apt install bubblewrap\n  - Fedora: sudo dnf install bubblewrap\n  - Arch: sudo pacman -S bubblewrap")]
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
