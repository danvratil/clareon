// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Artifact storage operations

use sqlx::Row;

use crate::error::Result;
use crate::types::{Artifact, ConversationId};

use super::Storage;

impl Storage {
    // ==================== Artifacts Operations ====================

    /// Add an artifact to the database
    pub async fn add_artifact(
        &self,
        conversation_id: &ConversationId,
        message_id: i64,
        filename: &str,
        mime_type: Option<&str>,
        content: &[u8],
    ) -> Result<i64> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        let content_hash = format!("{:x}", hasher.finalize());

        let size_bytes = content.len() as i64;
        let now = chrono::Utc::now().timestamp();

        let result = sqlx::query(
            r#"
            INSERT INTO artifacts
            (conversation_id, message_id, filename, mime_type, size_bytes,
             content, content_hash, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(conversation_id.as_ref())
        .bind(message_id)
        .bind(filename)
        .bind(mime_type)
        .bind(size_bytes)
        .bind(content)
        .bind(&content_hash)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Update an existing artifact
    pub async fn update_artifact(
        &self,
        conversation_id: &ConversationId,
        message_id: i64,
        filename: &str,
        content: &[u8],
    ) -> Result<()> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        let content_hash = format!("{:x}", hasher.finalize());

        let size_bytes = content.len() as i64;
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            UPDATE artifacts
            SET content = ?, content_hash = ?, size_bytes = ?,
                updated_at = ?, message_id = ?
            WHERE conversation_id = ? AND filename = ?
            "#,
        )
        .bind(content)
        .bind(&content_hash)
        .bind(size_bytes)
        .bind(now)
        .bind(message_id)
        .bind(conversation_id.as_ref())
        .bind(filename)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get all artifacts for a conversation
    pub async fn get_artifacts(&self, conversation_id: &ConversationId) -> Result<Vec<Artifact>> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, message_id, filename, mime_type,
                   size_bytes, content, content_hash, created_at, updated_at
            FROM artifacts
            WHERE conversation_id = ?
            ORDER BY updated_at DESC
            "#,
        )
        .bind(conversation_id.as_ref())
        .fetch_all(&self.pool)
        .await?;

        let mut artifacts = Vec::new();
        for row in rows {
            artifacts.push(Artifact {
                id: row.get("id"),
                conversation_id: row.get::<String, _>("conversation_id").into(),
                message_id: row.get("message_id"),
                filename: row.get("filename"),
                mime_type: row.get("mime_type"),
                size_bytes: row.get("size_bytes"),
                content: row.get("content"),
                content_hash: row.get("content_hash"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(artifacts)
    }

    /// Get artifacts for a specific message
    pub async fn get_artifacts_for_message(&self, message_id: i64) -> Result<Vec<Artifact>> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, message_id, filename, mime_type,
                   size_bytes, content, content_hash, created_at, updated_at
            FROM artifacts
            WHERE message_id = ?
            ORDER BY updated_at DESC
            "#,
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await?;

        let mut artifacts = Vec::new();
        for row in rows {
            artifacts.push(Artifact {
                id: row.get("id"),
                conversation_id: row.get::<String, _>("conversation_id").into(),
                message_id: row.get("message_id"),
                filename: row.get("filename"),
                mime_type: row.get("mime_type"),
                size_bytes: row.get("size_bytes"),
                content: row.get("content"),
                content_hash: row.get("content_hash"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(artifacts)
    }

    /// Get a single artifact by ID
    pub async fn get_artifact_by_id(&self, artifact_id: i64) -> Result<Artifact> {
        let row = sqlx::query(
            r#"
            SELECT id, conversation_id, message_id, filename, mime_type,
                   size_bytes, content, content_hash, created_at, updated_at
            FROM artifacts
            WHERE id = ?
            "#,
        )
        .bind(artifact_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Artifact {
            id: row.get("id"),
            conversation_id: row.get::<String, _>("conversation_id").into(),
            message_id: row.get("message_id"),
            filename: row.get("filename"),
            mime_type: row.get("mime_type"),
            size_bytes: row.get("size_bytes"),
            content: row.get("content"),
            content_hash: row.get("content_hash"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}
