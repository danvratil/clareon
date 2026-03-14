// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Message storage operations including user files and full-text search

use sqlx::Row;

use crate::error::{Error, Result};
use crate::types::{ContentBlock, ConversationId, Message, Role, SearchResult, UserFile};

use super::Storage;

impl Storage {
    // ==================== Message Operations ====================

    /// Add a message to a conversation
    pub async fn add_message(&self, message: &Message) -> Result<i64> {
        let content_json = serde_json::to_string(&message.content)?;

        let result = sqlx::query(
            r#"
            INSERT INTO messages (conversation_id, created_at, role, text_content, content_json, input_tokens, output_tokens, model)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(message.conversation_id.as_ref())
        .bind(message.created_at)
        .bind(message.role.as_str())
        .bind(&message.text_content)
        .bind(&content_json)
        .bind(message.input_tokens)
        .bind(message.output_tokens)
        .bind(&message.model)
        .execute(&self.pool)
        .await?;

        // Update conversation's updated_at timestamp
        sqlx::query("UPDATE conversations SET updated_at = ? WHERE id = ?")
            .bind(message.created_at)
            .bind(message.conversation_id.as_ref())
            .execute(&self.pool)
            .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get a message by ID
    pub async fn get_message(&self, id: i64) -> Result<Message> {
        let row = sqlx::query(
            r#"
            SELECT id, conversation_id, created_at, role, text_content, content_json, input_tokens, output_tokens, model
            FROM messages
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::MessageNotFound(id))?;

        self.row_to_message(row)
    }

    /// Get all messages for a conversation, ordered by creation time
    pub async fn get_messages(&self, conversation_id: &ConversationId) -> Result<Vec<Message>> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, created_at, role, text_content, content_json, input_tokens, output_tokens, model
            FROM messages
            WHERE conversation_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(conversation_id.as_ref())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| self.row_to_message(row))
            .collect()
    }

    /// Delete a message
    pub async fn delete_message(&self, id: i64) -> Result<()> {
        let result = sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(Error::MessageNotFound(id));
        }

        Ok(())
    }

    // ==================== Search Operations ====================

    /// Search messages using FTS5
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let rows = sqlx::query(
            r#"
            SELECT
                m.id as message_id,
                m.conversation_id,
                c.title as conversation_title,
                m.role,
                snippet(messages_fts, 0, '<mark>', '</mark>', '...', 32) as snippet,
                m.created_at
            FROM messages_fts
            JOIN messages m ON messages_fts.rowid = m.rowid
            JOIN conversations c ON m.conversation_id = c.id
            WHERE messages_fts MATCH ?
            ORDER BY rank
            LIMIT 50
            "#,
        )
        .bind(query)
        .fetch_all(&self.pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|row| SearchResult {
                conversation_id: row.get::<String, _>("conversation_id").into(),
                conversation_title: row.get("conversation_title"),
                message_id: row.get("message_id"),
                role: row.get("role"),
                snippet: row.get("snippet"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(results)
    }

    // ==================== User Files Operations ====================

    /// Add a user-uploaded file to the database
    pub async fn add_user_file(
        &self,
        conversation_id: &ConversationId,
        message_id: i64,
        filename: &str,
        mime_type: &str,
        content: &[u8],
    ) -> Result<i64> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content);
        let content_hash = format!("{:x}", hasher.finalize());

        let size_bytes = content.len() as i64;
        let created_at = chrono::Utc::now().timestamp();

        let result = sqlx::query(
            r#"
            INSERT INTO user_files
            (conversation_id, message_id, filename, mime_type, size_bytes, content, content_hash, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(conversation_id.as_ref())
        .bind(message_id)
        .bind(filename)
        .bind(mime_type)
        .bind(size_bytes)
        .bind(content)
        .bind(&content_hash)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get all user files for a conversation
    pub async fn get_user_files(&self, conversation_id: &ConversationId) -> Result<Vec<UserFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, message_id, filename, mime_type,
                   size_bytes, content, content_hash, created_at
            FROM user_files
            WHERE conversation_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(conversation_id.as_ref())
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(UserFile {
                id: row.get("id"),
                conversation_id: row.get::<String, _>("conversation_id").into(),
                message_id: row.get("message_id"),
                filename: row.get("filename"),
                mime_type: row.get("mime_type"),
                size_bytes: row.get("size_bytes"),
                content: row.get("content"),
                content_hash: row.get("content_hash"),
                created_at: row.get("created_at"),
            });
        }

        Ok(files)
    }

    /// Get user files for a specific message
    pub async fn get_user_files_for_message(&self, message_id: i64) -> Result<Vec<UserFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, message_id, filename, mime_type,
                   size_bytes, content, content_hash, created_at
            FROM user_files
            WHERE message_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(UserFile {
                id: row.get("id"),
                conversation_id: row.get::<String, _>("conversation_id").into(),
                message_id: row.get("message_id"),
                filename: row.get("filename"),
                mime_type: row.get("mime_type"),
                size_bytes: row.get("size_bytes"),
                content: row.get("content"),
                content_hash: row.get("content_hash"),
                created_at: row.get("created_at"),
            });
        }

        Ok(files)
    }

    // ==================== Helper Methods ====================

    fn row_to_message(&self, row: sqlx::sqlite::SqliteRow) -> Result<Message> {
        let role_str: String = row.get("role");
        let role: Role = role_str
            .parse()
            .map_err(|e: String| Error::Database(sqlx::Error::Protocol(e)))?;

        let content_json: String = row.get("content_json");
        let content: Vec<ContentBlock> = serde_json::from_str(&content_json)?;

        Ok(Message {
            id: row.get("id"),
            conversation_id: row.get::<String, _>("conversation_id").into(),
            created_at: row.get("created_at"),
            role,
            text_content: row.get("text_content"),
            content,
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            model: row.get("model"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Conversation;

    #[tokio::test]
    async fn test_add_and_get_messages() {
        let storage = Storage::in_memory().await.unwrap();

        let conv = Conversation::new("test");
        let conv_id = storage.create_conversation(&conv).await.unwrap();

        let msg = Message::user(conv_id.clone(), "Hello!");
        let msg_id = storage.add_message(&msg).await.unwrap();

        let loaded = storage.get_message(msg_id).await.unwrap();
        assert_eq!(loaded.text_content, Some("Hello!".to_string()));
        assert_eq!(loaded.role, Role::User);
    }

    #[tokio::test]
    async fn test_get_messages_for_conversation() {
        let storage = Storage::in_memory().await.unwrap();

        let conv = Conversation::new("test");
        let conv_id = storage.create_conversation(&conv).await.unwrap();

        let msg1 = Message::user(conv_id.clone(), "First message");
        let msg2 = Message::user(conv_id.clone(), "Second message");

        storage.add_message(&msg1).await.unwrap();
        storage.add_message(&msg2).await.unwrap();

        let messages = storage.get_messages(&conv_id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text_content, Some("First message".to_string()));
        assert_eq!(messages[1].text_content, Some("Second message".to_string()));
    }

    #[tokio::test]
    async fn test_search_messages() {
        let storage = Storage::in_memory().await.unwrap();

        let conv = Conversation::new("test");
        let conv_id = storage.create_conversation(&conv).await.unwrap();

        let msg1 = Message::user(conv_id.clone(), "Hello world");
        let msg2 = Message::user(conv_id.clone(), "Goodbye world");
        let msg3 = Message::user(conv_id.clone(), "Something else");

        storage.add_message(&msg1).await.unwrap();
        storage.add_message(&msg2).await.unwrap();
        storage.add_message(&msg3).await.unwrap();

        let results = storage.search("world").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_cascade_delete() {
        let storage = Storage::in_memory().await.unwrap();

        let conv = Conversation::new("test");
        let conv_id = storage.create_conversation(&conv).await.unwrap();

        let msg = Message::user(conv_id.clone(), "Test message");
        let msg_id = storage.add_message(&msg).await.unwrap();

        // Delete conversation should also delete messages
        storage.delete_conversation(&conv_id).await.unwrap();

        let result = storage.get_message(msg_id).await;
        assert!(matches!(result, Err(Error::MessageNotFound(_))));
    }
}
