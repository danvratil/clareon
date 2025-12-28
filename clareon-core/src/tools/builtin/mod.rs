// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

mod list_directory;
mod read_file;
mod write_file;

pub use list_directory::ListDirectoryTool;
pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;

use std::sync::Arc;

use super::ToolRegistry;

/// Register all built-in tools with the registry
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    registry.register(Arc::new(ReadFileTool));
    registry.register(Arc::new(WriteFileTool));
    registry.register(Arc::new(ListDirectoryTool));
}
