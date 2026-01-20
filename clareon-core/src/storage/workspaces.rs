// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Workspace metadata storage operations

use sqlx::Row;

use crate::error::Result;
use crate::types::{ConversationId, WorkspaceMetadata};

use super::Storage;

impl Storage {
    // ==================== Workspace Metadata Operations ====================

    /// Create workspace metadata for a conversation
    pub async fn create_workspace_metadata(
        &self,
        conversation_id: &ConversationId,
        workspace_path: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO workspace_metadata
            (conversation_id, workspace_path, created_at, last_accessed_at, disk_usage_bytes)
            VALUES (?, ?, ?, ?, 0)
            "#,
        )
        .bind(conversation_id.as_ref())
        .bind(workspace_path)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get workspace metadata for a conversation
    pub async fn get_workspace_metadata(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Option<WorkspaceMetadata>> {
        let row = sqlx::query(
            r#"
            SELECT conversation_id, workspace_path, created_at, last_accessed_at,
                   installed_packages, disk_usage_bytes
            FROM workspace_metadata
            WHERE conversation_id = ?
            "#,
        )
        .bind(conversation_id.as_ref())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(WorkspaceMetadata {
                conversation_id: row.get::<String, _>("conversation_id").into(),
                workspace_path: row.get("workspace_path"),
                created_at: row.get("created_at"),
                last_accessed_at: row.get("last_accessed_at"),
                installed_packages: row.get("installed_packages"),
                disk_usage_bytes: row.get("disk_usage_bytes"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Update workspace last access time
    pub async fn update_workspace_last_access(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE workspace_metadata
            SET last_accessed_at = ?
            WHERE conversation_id = ?
            "#,
        )
        .bind(now)
        .bind(conversation_id.as_ref())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update workspace disk usage
    pub async fn update_workspace_disk_usage(
        &self,
        conversation_id: &ConversationId,
        bytes: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE workspace_metadata
            SET disk_usage_bytes = ?
            WHERE conversation_id = ?
            "#,
        )
        .bind(bytes)
        .bind(conversation_id.as_ref())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update installed packages list
    pub async fn update_workspace_packages(
        &self,
        conversation_id: &ConversationId,
        packages_json: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE workspace_metadata
            SET installed_packages = ?
            WHERE conversation_id = ?
            "#,
        )
        .bind(packages_json)
        .bind(conversation_id.as_ref())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete workspace metadata
    pub async fn delete_workspace_metadata(&self, conversation_id: &ConversationId) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM workspace_metadata
            WHERE conversation_id = ?
            "#,
        )
        .bind(conversation_id.as_ref())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get workspaces older than a timestamp
    pub async fn get_workspaces_older_than(
        &self,
        timestamp: i64,
    ) -> Result<Vec<WorkspaceMetadata>> {
        let rows = sqlx::query(
            r#"
            SELECT conversation_id, workspace_path, created_at, last_accessed_at,
                   installed_packages, disk_usage_bytes
            FROM workspace_metadata
            WHERE last_accessed_at < ?
            ORDER BY last_accessed_at ASC
            "#,
        )
        .bind(timestamp)
        .fetch_all(&self.pool)
        .await?;

        let mut metadata = Vec::new();
        for row in rows {
            metadata.push(WorkspaceMetadata {
                conversation_id: row.get::<String, _>("conversation_id").into(),
                workspace_path: row.get("workspace_path"),
                created_at: row.get("created_at"),
                last_accessed_at: row.get("last_accessed_at"),
                installed_packages: row.get("installed_packages"),
                disk_usage_bytes: row.get("disk_usage_bytes"),
            });
        }

        Ok(metadata)
    }
}
