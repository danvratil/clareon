//! Workspace management for persistent conversation workspaces

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::storage::Storage;
use crate::types::UserFile;

use super::ToolError;

/// Persistent workspace for a conversation (does NOT auto-cleanup on drop)
#[derive(Debug, Clone)]
pub struct PersistentWorkspace {
    conversation_id: i64,
    root_path: PathBuf,           // ~/.cache/clareon/conversations/<id>/
    workspace_path: PathBuf,      // ~/.cache/clareon/conversations/<id>/workspace/
    input_path: PathBuf,          // ~/.cache/clareon/conversations/<id>/input/
    output_path: PathBuf,         // ~/.cache/clareon/conversations/<id>/output/
    pip_cache_path: PathBuf,      // ~/.cache/clareon/shared/pip/
}

impl PersistentWorkspace {
    /// Create a new persistent workspace for a conversation
    pub fn new(conversation_id: i64, cache_root: &Path, shared_pip: &Path) -> std::result::Result<Self, ToolError> {
        let root_path = cache_root.join("conversations").join(conversation_id.to_string());
        let workspace_path = root_path.join("workspace");
        let input_path = root_path.join("input");
        let output_path = root_path.join("output");
        let pip_cache_path = shared_pip.to_path_buf();

        Ok(Self {
            conversation_id,
            root_path,
            workspace_path,
            input_path,
            output_path,
            pip_cache_path,
        })
    }

    /// Ensure all directories exist
    pub async fn ensure_directories(&self) -> std::result::Result<(), ToolError> {
        fs::create_dir_all(&self.workspace_path)
            .await
            .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;

        fs::create_dir_all(&self.input_path)
            .await
            .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;

        fs::create_dir_all(&self.output_path)
            .await
            .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;

        fs::create_dir_all(&self.pip_cache_path)
            .await
            .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;

        debug!(
            "Ensured workspace directories for conversation {}",
            self.conversation_id
        );

        Ok(())
    }

    /// Get the workspace path (mounted to /home/claude)
    pub fn workspace(&self) -> &Path {
        &self.workspace_path
    }

    /// Get the input path (mounted RO to /mnt/user-data/uploads)
    pub fn input(&self) -> &Path {
        &self.input_path
    }

    /// Get the output path (mounted RW to /mnt/user-data/outputs)
    pub fn output(&self) -> &Path {
        &self.output_path
    }

    /// Get the shared pip cache path (mounted to /home/claude/.local/)
    pub fn pip_cache(&self) -> &Path {
        &self.pip_cache_path
    }

    /// Get the root path
    pub fn root(&self) -> &Path {
        &self.root_path
    }

    /// Extract user files from database to input directory
    pub async fn populate_input_files(&self, files: &[UserFile]) -> std::result::Result<(), ToolError> {
        for file in files {
            let file_path = self.input_path.join(&file.filename);

            // Create parent directories if needed
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;
            }

            // Write file content
            fs::write(&file_path, &file.content)
                .await
                .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;

            debug!("Extracted user file {} to input directory", file.filename);
        }

        Ok(())
    }

    /// Scan output directory and return list of new/modified files
    ///
    /// Returns (relative_path, content) pairs for files that are new or have changed
    pub async fn collect_artifacts(
        &self,
        previous_hashes: &HashMap<String, String>,
    ) -> std::result::Result<Vec<(String, Vec<u8>)>, ToolError> {
        use sha2::{Digest, Sha256};

        let mut artifacts = Vec::new();

        // Recursively scan output directory
        let mut stack = vec![self.output_path.clone()];

        while let Some(current_dir) = stack.pop() {
            let mut entries = fs::read_dir(&current_dir)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            {
                let path = entry.path();
                let metadata = entry
                    .metadata()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                if metadata.is_dir() {
                    stack.push(path);
                } else if metadata.is_file() {
                    // Get relative path
                    let relative_path = path
                        .strip_prefix(&self.output_path)
                        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
                        .to_string_lossy()
                        .to_string();

                    // Read file content
                    let content = fs::read(&path)
                        .await
                        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                    // Compute hash
                    let mut hasher = Sha256::new();
                    hasher.update(&content);
                    let hash = format!("{:x}", hasher.finalize());

                    // Check if file is new or modified
                    if previous_hashes.get(&relative_path) != Some(&hash) {
                        artifacts.push((relative_path, content));
                    }
                }
            }
        }

        Ok(artifacts)
    }

    /// Calculate disk usage for workspace (in bytes)
    pub async fn calculate_disk_usage(&self) -> std::result::Result<u64, ToolError> {
        let mut total_size: u64 = 0;

        // Calculate size of all three directories
        for dir in [&self.workspace_path, &self.input_path, &self.output_path] {
            if dir.exists() {
                total_size += Self::calculate_dir_size(dir).await?;
            }
        }

        Ok(total_size)
    }

    /// Recursively calculate directory size
    async fn calculate_dir_size(path: &Path) -> std::result::Result<u64, ToolError> {
        let mut size: u64 = 0;
        let mut stack = vec![path.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            let mut entries = fs::read_dir(&current_dir)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            {
                let metadata = entry
                    .metadata()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                if metadata.is_dir() {
                    stack.push(entry.path());
                } else if metadata.is_file() {
                    size += metadata.len();
                }
            }
        }

        Ok(size)
    }

    /// Clean workspace (delete all contents, for manual cleanup)
    pub async fn clean(&self) -> std::result::Result<(), ToolError> {
        if self.root_path.exists() {
            fs::remove_dir_all(&self.root_path)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        }
        Ok(())
    }
}

/// Manages workspace lifecycle and caching
pub struct WorkspaceManager {
    cache_root: PathBuf,              // ~/.cache/clareon/
    shared_pip: PathBuf,              // ~/.cache/clareon/shared/pip/
    pub(crate) storage: Arc<Storage>, // Make accessible to ToolExecutor

    // Cache of loaded workspaces (conversation_id -> workspace)
    workspaces: Arc<RwLock<HashMap<i64, Arc<PersistentWorkspace>>>>,
}

impl WorkspaceManager {
    pub fn new(cache_root: PathBuf, storage: Arc<Storage>) -> Self {
        let shared_pip = cache_root.join("shared").join("pip");

        Self {
            cache_root,
            shared_pip,
            storage,
            workspaces: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Ensure shared directories exist (shared/pip/)
    pub async fn ensure_shared_directories(&self) -> std::result::Result<(), ToolError> {
        fs::create_dir_all(&self.shared_pip)
            .await
            .map_err(|e| ToolError::WorkspaceCreationFailed(e.to_string()))?;

        info!("Ensured shared pip cache directory: {:?}", self.shared_pip);
        Ok(())
    }

    /// Get or create workspace for a conversation
    pub async fn get_workspace(&self, conversation_id: i64) -> std::result::Result<Arc<PersistentWorkspace>, ToolError> {
        // Check cache first
        {
            let workspaces = self.workspaces.read().await;
            if let Some(workspace) = workspaces.get(&conversation_id) {
                // Update last access time
                if let Err(e) = self.storage.update_workspace_last_access(conversation_id).await {
                    warn!("Failed to update workspace last access: {}", e);
                }
                return Ok(workspace.clone());
            }
        }

        // Load or create workspace
        let workspace = self.load_or_create_workspace(conversation_id).await?;

        // Cache it
        {
            let mut workspaces = self.workspaces.write().await;
            workspaces.insert(conversation_id, workspace.clone());
        }

        Ok(workspace)
    }

    /// Cleanup workspaces older than N days
    pub async fn cleanup_old_workspaces(&self, days: u64) -> std::result::Result<Vec<i64>, ToolError> {
        use chrono::Utc;

        let cutoff_timestamp = Utc::now().timestamp() - (days * 86400) as i64;

        let old_workspaces = self
            .storage
            .get_workspaces_older_than(cutoff_timestamp)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let mut cleaned = Vec::new();

        for metadata in old_workspaces {
            let workspace_path = Path::new(&metadata.workspace_path);

            // Delete directory if it exists
            if workspace_path.exists() {
                if let Err(e) = fs::remove_dir_all(workspace_path).await {
                    warn!(
                        "Failed to delete workspace directory for conversation {}: {}",
                        metadata.conversation_id, e
                    );
                    continue;
                }
            }

            // Delete database record
            if let Err(e) = self
                .storage
                .delete_workspace_metadata(metadata.conversation_id)
                .await
            {
                warn!(
                    "Failed to delete workspace metadata for conversation {}: {}",
                    metadata.conversation_id, e
                );
                continue;
            }

            // Remove from cache
            {
                let mut workspaces = self.workspaces.write().await;
                workspaces.remove(&metadata.conversation_id);
            }

            cleaned.push(metadata.conversation_id);
        }

        info!("Cleaned up {} old workspaces", cleaned.len());
        Ok(cleaned)
    }

    /// Get workspace for conversation (from cache or create new)
    async fn load_or_create_workspace(
        &self,
        conversation_id: i64,
    ) -> std::result::Result<Arc<PersistentWorkspace>, ToolError> {
        // Check database first
        let metadata = self
            .storage
            .get_workspace_metadata(conversation_id)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let workspace = if let Some(_metadata) = metadata {
            // Workspace exists in database
            let ws = PersistentWorkspace::new(conversation_id, &self.cache_root, &self.shared_pip)?;

            // Verify directory exists, recreate if missing
            if !ws.root().exists() {
                warn!(
                    "Workspace directory missing for conversation {}, recreating",
                    conversation_id
                );
                ws.ensure_directories().await?;
            }

            ws
        } else {
            // Create new workspace
            let ws = PersistentWorkspace::new(conversation_id, &self.cache_root, &self.shared_pip)?;
            ws.ensure_directories().await?;

            // Store metadata in database
            self.storage
                .create_workspace_metadata(
                    conversation_id,
                    ws.root().to_string_lossy().as_ref(),
                )
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            info!("Created new workspace for conversation {}", conversation_id);
            ws
        };

        // Update last access time
        if let Err(e) = self.storage.update_workspace_last_access(conversation_id).await {
            warn!("Failed to update workspace last access: {}", e);
        }

        Ok(Arc::new(workspace))
    }
}
