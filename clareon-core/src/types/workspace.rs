//! Workspace-related types for file management and artifact tracking

use serde::{Deserialize, Serialize};

/// User-uploaded file stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFile {
    pub id: i64,
    pub conversation_id: i64,
    pub message_id: i64,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    #[serde(skip)]  // Don't serialize content by default (can be large)
    pub content: Vec<u8>,
    pub content_hash: String,  // SHA-256
    pub created_at: i64,
}

/// Claude-generated artifact (file in output directory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub conversation_id: i64,
    pub message_id: i64,
    pub filename: String,       // Relative path in output/
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    #[serde(skip)]  // Don't serialize content by default (can be large)
    pub content: Vec<u8>,
    pub content_hash: String,  // SHA-256
    pub created_at: i64,
    pub updated_at: i64,
}

/// Workspace metadata for a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub conversation_id: i64,
    pub workspace_path: String,
    pub created_at: i64,
    pub last_accessed_at: i64,
    pub installed_packages: Option<String>,  // JSON array
    pub disk_usage_bytes: i64,
}
