// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conversation storage operations

use sqlx::Row;

use crate::error::{Error, Result};
use crate::types::{Conversation, ConversationId, ConversationSummary};

use super::Storage;

impl Storage {
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
    async fn test_list_conversations() {
        let storage = Storage::in_memory().await.unwrap();

        let conv1 = Conversation::new("model1");
        let conv2 = Conversation::new("model2");

        storage.create_conversation(&conv1).await.unwrap();
        storage.create_conversation(&conv2).await.unwrap();

        let summaries = storage.list_conversations().await.unwrap();
        assert_eq!(summaries.len(), 2);
    }
}
