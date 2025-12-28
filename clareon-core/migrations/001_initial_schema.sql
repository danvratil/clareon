-- SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
--
-- SPDX-License-Identifier: GPL-3.0-or-later

-- Initial schema for Clareon
-- Creates conversations, messages, workspaces, artifacts, and FTS support

-- Conversations table with UUID primary key
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,  -- UUIDv4 string
    title TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    model TEXT NOT NULL,
    system_prompt TEXT,
    custom_instructions TEXT
);

CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);

-- Messages table
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    text_content TEXT,
    content_json TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    model TEXT
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id, created_at);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE messages_fts USING fts5(
    text_content,
    content='messages',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync with messages table
CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, text_content)
    VALUES (new.rowid, new.text_content);
END;

CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, text_content)
    VALUES ('delete', old.rowid, old.text_content);
END;

CREATE TRIGGER messages_fts_update AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, text_content)
    VALUES ('delete', old.rowid, old.text_content);
    INSERT INTO messages_fts(rowid, text_content)
    VALUES (new.rowid, new.text_content);
END;

-- Summaries table
CREATE TABLE conversation_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    summary TEXT NOT NULL,
    summarized_from INTEGER NOT NULL,
    summarized_to INTEGER NOT NULL,
    message_count INTEGER NOT NULL,
    token_count INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_summaries_conversation ON conversation_summaries(conversation_id);

-- User files
CREATE TABLE user_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL
        REFERENCES messages(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(conversation_id, message_id, filename)
);

CREATE INDEX idx_user_files_conversation ON user_files(conversation_id);
CREATE INDEX idx_user_files_message ON user_files(message_id);
CREATE INDEX idx_user_files_hash ON user_files(content_hash);

-- Artifacts
CREATE TABLE artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL
        REFERENCES messages(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    mime_type TEXT,
    size_bytes INTEGER NOT NULL,
    content BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(conversation_id, filename, created_at)
);

CREATE INDEX idx_artifacts_conversation ON artifacts(conversation_id);
CREATE INDEX idx_artifacts_message ON artifacts(message_id);
CREATE INDEX idx_artifacts_hash ON artifacts(content_hash);

-- Workspace metadata
CREATE TABLE workspace_metadata (
    conversation_id TEXT PRIMARY KEY
        REFERENCES conversations(id) ON DELETE CASCADE,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,
    installed_packages TEXT,
    disk_usage_bytes INTEGER DEFAULT 0
);

CREATE INDEX idx_workspace_last_access ON workspace_metadata(last_accessed_at);
