// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

-- Migration 002: Persistent Workspaces
-- Add support for user file uploads, artifact management, and persistent workspaces

-- User uploaded files (stored as blobs)
CREATE TABLE IF NOT EXISTS user_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL
        REFERENCES messages(id) ON DELETE CASCADE,

    -- File metadata
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,

    -- Content (stored as blob for binary safety)
    content BLOB NOT NULL,

    -- Hash for deduplication
    content_hash TEXT NOT NULL,  -- SHA-256

    created_at INTEGER NOT NULL,

    UNIQUE(conversation_id, message_id, filename)
);

CREATE INDEX IF NOT EXISTS idx_user_files_conversation ON user_files(conversation_id);
CREATE INDEX IF NOT EXISTS idx_user_files_message ON user_files(message_id);
CREATE INDEX IF NOT EXISTS idx_user_files_hash ON user_files(content_hash);

-- Claude-generated artifacts (files created in output directory)
CREATE TABLE IF NOT EXISTS artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL
        REFERENCES conversations(id) ON DELETE CASCADE,
    message_id INTEGER NOT NULL
        REFERENCES messages(id) ON DELETE CASCADE,

    -- File metadata
    filename TEXT NOT NULL,       -- Relative path in output/ directory
    mime_type TEXT,               -- Detected MIME type
    size_bytes INTEGER NOT NULL,

    -- Content (stored as blob)
    content BLOB NOT NULL,

    -- Hash for deduplication/change detection
    content_hash TEXT NOT NULL,   -- SHA-256

    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    UNIQUE(conversation_id, filename, created_at)
);

CREATE INDEX IF NOT EXISTS idx_artifacts_conversation ON artifacts(conversation_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_message ON artifacts(message_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_hash ON artifacts(content_hash);

-- Workspace metadata: Track workspace state per conversation
CREATE TABLE IF NOT EXISTS workspace_metadata (
    conversation_id INTEGER PRIMARY KEY
        REFERENCES conversations(id) ON DELETE CASCADE,

    -- Workspace paths
    workspace_path TEXT NOT NULL,     -- Absolute path to workspace directory

    -- State tracking
    created_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,

    -- Pip packages installed (JSON array of package names)
    installed_packages TEXT,          -- JSON: ["numpy", "pandas", ...]

    -- Total disk usage (bytes)
    disk_usage_bytes INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_workspace_last_access ON workspace_metadata(last_accessed_at);
