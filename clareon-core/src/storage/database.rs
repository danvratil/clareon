// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! SQLite database operations

use sqlx::{Pool, Row, Sqlite, sqlite::SqlitePoolOptions};
use tracing::{debug, info};

use crate::error::{Error, Result};
use crate::types::{
    Artifact, ContentBlock, Conversation, ConversationId, ConversationSummary, Message, Role,
    SearchResult, UserFile, WorkspaceMetadata,
};

/// Storage layer for persisting conversations and messages
pub struct Storage {
    pool: Pool<Sqlite>,
}

impl Storage {
    /// Create a new storage instance and initialize the database
    ///
    /// # Arguments
    /// * `database_url` - SQLite database URL (e.g., "sqlite:///path/to/db.sqlite" or "sqlite::memory:")
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to database: {}", database_url);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let storage = Self { pool };
        storage.run_migrations().await?;

        Ok(storage)
    }

    /// Create an in-memory storage instance (useful for testing)
    pub async fn in_memory() -> Result<Self> {
        Self::new("sqlite::memory:").await
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<()> {
        debug!("Running database migrations");

        sqlx::migrate!("./migrations").run(&self.pool).await?;

        info!("Database migrations completed");
        Ok(())
    }

    // ==================== Conversation Operations ====================

    /// Create a new conversation
    pub async fn create_conversation(&self, conversation: &Conversation) -> Result<ConversationId> {
        sqlx::query(
            r#"
            INSERT INTO conversations (id, title, created_at, updated_at, model, system_prompt, custom_instructions)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(conversation.id.as_ref())
        .bind(&conversation.title)
        .bind(conversation.created_at)
        .bind(conversation.updated_at)
        .bind(&conversation.model)
        .bind(&conversation.system_prompt)
        .bind(&conversation.custom_instructions)
        .execute(&self.pool)
        .await?;

        Ok(conversation.id.clone())
    }

    /// Get a conversation by ID
    pub async fn get_conversation(&self, id: &ConversationId) -> Result<Conversation> {
        let row = sqlx::query(
            r#"
            SELECT id, title, created_at, updated_at, model, system_prompt, custom_instructions
            FROM conversations
            WHERE id = ?
            "#,
        )
        .bind(id.as_ref())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::ConversationNotFound(id.clone()))?;

        Ok(Conversation {
            id: row.get::<String, _>("id").into(),
            title: row.get("title"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            model: row.get("model"),
            system_prompt: row.get("system_prompt"),
            custom_instructions: row.get("custom_instructions"),
        })
    }

    /// Update a conversation
    pub async fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE conversations
            SET title = ?, updated_at = ?, model = ?, system_prompt = ?, custom_instructions = ?
            WHERE id = ?
            "#,
        )
        .bind(&conversation.title)
        .bind(conversation.updated_at)
        .bind(&conversation.model)
        .bind(&conversation.system_prompt)
        .bind(&conversation.custom_instructions)
        .bind(conversation.id.as_ref())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(Error::ConversationNotFound(conversation.id.clone()));
        }

        Ok(())
    }

    /// Delete a conversation and all its messages
    pub async fn delete_conversation(&self, id: &ConversationId) -> Result<()> {
        let result = sqlx::query("DELETE FROM conversations WHERE id = ?")
            .bind(id.as_ref())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(Error::ConversationNotFound(id.clone()));
        }

        Ok(())
    }

    /// List all conversations, ordered by most recently updated
    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let rows = sqlx::query(
            r#"
            SELECT
                c.id,
                c.title,
                c.updated_at,
                c.model,
                COUNT(m.id) as message_count
            FROM conversations c
            LEFT JOIN messages m ON m.conversation_id = c.id
            GROUP BY c.id
            ORDER BY c.updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let summaries = rows
            .into_iter()
            .map(|row| ConversationSummary {
                id: row.get::<String, _>("id").into(),
                title: row.get("title"),
                updated_at: row.get("updated_at"),
                model: row.get("model"),
                message_count: row.get("message_count"),
            })
            .collect();

        Ok(summaries)
    }

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

    /// Get token counts for a conversation
    pub async fn get_conversation_token_count(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<(i64, i64)> {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(input_tokens), 0) as total_input,
                COALESCE(SUM(output_tokens), 0) as total_output
            FROM messages
            WHERE conversation_id = ?
            "#,
        )
        .bind(conversation_id.as_ref())
        .fetch_one(&self.pool)
        .await?;

        Ok((row.get("total_input"), row.get("total_output")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_conversation() {
        let storage = match Storage::in_memory().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create storage: {}", e);
                eprintln!("Error type: {:?}", e);
                panic!("Storage creation failed");
            }
        };

        let conv = Conversation::new("claude-sonnet-4-20250514");
        let id = storage.create_conversation(&conv).await.unwrap();

        let loaded = storage.get_conversation(&id).await.unwrap();
        assert_eq!(loaded.model, "claude-sonnet-4-20250514");
        assert!(loaded.title.is_none());
    }

    #[tokio::test]
    async fn test_update_conversation() {
        let storage = Storage::in_memory().await.unwrap();

        let mut conv = Conversation::new("claude-sonnet-4-20250514");
        let id = storage.create_conversation(&conv).await.unwrap();

        conv.id = id.clone();
        conv.set_title("My Chat");
        storage.update_conversation(&conv).await.unwrap();

        let loaded = storage.get_conversation(&id).await.unwrap();
        assert_eq!(loaded.title, Some("My Chat".to_string()));
    }

    #[tokio::test]
    async fn test_delete_conversation() {
        let storage = Storage::in_memory().await.unwrap();

        let conv = Conversation::new("test");
        let id = storage.create_conversation(&conv).await.unwrap();

        storage.delete_conversation(&id).await.unwrap();

        let result = storage.get_conversation(&id).await;
        assert!(matches!(result, Err(Error::ConversationNotFound(_))));
    }

    #[tokio::test]
    async fn test_add_and_get_messages() {
        let storage = Storage::in_memory().await.unwrap();

        let conv = Conversation::new("test");
        let conv_id = storage.create_conversation(&conv).await.unwrap();

        let msg = Message::user(conv_id.clone(), "Hello, Claude!");
        let msg_id = storage.add_message(&msg).await.unwrap();

        let loaded = storage.get_message(msg_id).await.unwrap();
        assert_eq!(loaded.text_content, Some("Hello, Claude!".to_string()));
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
    async fn test_list_conversations() {
        let storage = Storage::in_memory().await.unwrap();

        let conv1 = Conversation::new("model1");
        let conv2 = Conversation::new("model2");

        storage.create_conversation(&conv1).await.unwrap();
        storage.create_conversation(&conv2).await.unwrap();

        let summaries = storage.list_conversations().await.unwrap();
        assert_eq!(summaries.len(), 2);
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
