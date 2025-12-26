-- Initial schema for Clareon
-- Creates conversations, messages, and FTS support

-- Conversations table
CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    model TEXT NOT NULL,
    system_prompt TEXT,
    custom_instructions TEXT
);

CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at DESC);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    text_content TEXT,
    content_json TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    model TEXT
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    text_content,
    content='messages',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync with messages table
CREATE TRIGGER IF NOT EXISTS messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, text_content)
    VALUES (new.rowid, new.text_content);
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_delete AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, text_content)
    VALUES ('delete', old.rowid, old.text_content);
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_update AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, text_content)
    VALUES ('delete', old.rowid, old.text_content);
    INSERT INTO messages_fts(rowid, text_content)
    VALUES (new.rowid, new.text_content);
END;

-- Artifacts table (for future use)
CREATE TABLE IF NOT EXISTS artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL
        REFERENCES messages(id) ON DELETE CASCADE,
    artifact_type TEXT NOT NULL,
    title TEXT,
    language TEXT,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_conversation ON artifacts(conversation_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_message ON artifacts(message_id);

-- Attachments table (for future use)
CREATE TABLE IF NOT EXISTS attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL
        REFERENCES messages(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content BLOB NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);

-- Summaries table (for future context management)
CREATE TABLE IF NOT EXISTS conversation_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    summary TEXT NOT NULL,
    summarized_from INTEGER NOT NULL,
    summarized_to INTEGER NOT NULL,
    message_count INTEGER NOT NULL,
    token_count INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_summaries_conversation ON conversation_summaries(conversation_id);
