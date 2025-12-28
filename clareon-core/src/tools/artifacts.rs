//! Artifact management for file synchronization between filesystem and database

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, info};

use crate::storage::Storage;

use super::{PersistentWorkspace, ToolError};

/// Manages artifact synchronization between filesystem and database
pub struct ArtifactManager {
    storage: Arc<Storage>,
}

impl ArtifactManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// Compute SHA-256 hash of file contents
    fn compute_hash(content: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }

    /// Scan output directory and return map of filename -> hash
    pub async fn scan_output_directory(
        output_path: &Path,
    ) -> Result<HashMap<String, String>, ToolError> {
        let mut hashes = HashMap::new();

        if !output_path.exists() {
            return Ok(hashes);
        }

        // Recursively scan directory
        let mut stack = vec![output_path.to_path_buf()];

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
                        .strip_prefix(output_path)
                        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
                        .to_string_lossy()
                        .to_string();

                    // Read and hash file
                    let content = fs::read(&path)
                        .await
                        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                    let hash = Self::compute_hash(&content);
                    hashes.insert(relative_path, hash);
                }
            }
        }

        Ok(hashes)
    }

    /// Synchronize artifacts from output directory to database
    ///
    /// Returns number of new/updated artifacts
    pub async fn sync_artifacts(
        &self,
        conversation_id: i64,
        message_id: i64,
        workspace: &PersistentWorkspace,
        previous_hashes: &HashMap<String, String>,
    ) -> Result<usize, ToolError> {
        // Collect new/modified artifacts
        let artifacts = workspace.collect_artifacts(previous_hashes).await?;

        let count = artifacts.len();

        for (filename, content) in artifacts {
            // Detect MIME type
            let mime_type = mime_guess::from_path(&filename)
                .first()
                .map(|m| m.essence_str().to_string());

            // Check if artifact already exists in database
            let existing_artifacts = self
                .storage
                .get_artifacts(conversation_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            let exists = existing_artifacts.iter().any(|a| a.filename == filename);

            if exists {
                // Update existing artifact
                self.storage
                    .update_artifact(conversation_id, message_id, &filename, &content)
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                debug!("Updated artifact: {}", filename);
            } else {
                // Add new artifact
                self.storage
                    .add_artifact(
                        conversation_id,
                        message_id,
                        &filename,
                        mime_type.as_deref(),
                        &content,
                    )
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

                debug!("Added new artifact: {}", filename);
            }
        }

        if count > 0 {
            info!(
                "Synchronized {} artifacts for conversation {}",
                count, conversation_id
            );
        }

        Ok(count)
    }

    /// Restore artifacts from database to output directory
    ///
    /// This is useful for recovering the output directory if it gets deleted
    pub async fn restore_artifacts(
        &self,
        conversation_id: i64,
        workspace: &PersistentWorkspace,
    ) -> Result<usize, ToolError> {
        let artifacts = self
            .storage
            .get_artifacts(conversation_id)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let count = artifacts.len();

        for artifact in artifacts {
            let file_path = workspace.output().join(&artifact.filename);

            // Create parent directories if needed
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            }

            // Write artifact content
            fs::write(&file_path, &artifact.content)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            debug!("Restored artifact: {}", artifact.filename);
        }

        if count > 0 {
            info!(
                "Restored {} artifacts for conversation {}",
                count, conversation_id
            );
        }

        Ok(count)
    }
}
