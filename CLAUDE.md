# Clareon

A Claude desktop assistant for Linux, designed as an alternative to Claude Desktop with AWS Bedrock and Anthropic API support.

## Project Overview

Clareon is a cross-platform Claude assistant with a focus on KDE Plasma integration (future). It provides a conversational interface to Claude models via AWS Bedrock or the Anthropic API, with local conversation storage, search, and future MCP/tool support.

## Architecture

```
clareon/
├── Cargo.toml                    # Workspace root
├── clareon-core/                 # Core library
│   └── src/
│       ├── lib.rs                # Library entry point
│       ├── error.rs              # Error types (Error, BackendError, ConfigError)
│       ├── types/
│       │   ├── mod.rs
│       │   ├── content.rs        # ContentBlock, ToolResultContent
│       │   ├── message.rs        # Message, Role
│       │   └── conversation.rs   # Conversation, ConversationSummary, SearchResult
│       ├── backend/
│       │   ├── mod.rs
│       │   ├── traits.rs         # LlmBackend trait, ChatRequest, ChatResponse
│       │   ├── anthropic.rs      # Anthropic API implementation
│       │   └── bedrock.rs        # AWS Bedrock implementation
│       ├── storage/
│       │   ├── mod.rs
│       │   └── database.rs       # SQLite with FTS5 search
│       ├── config/
│       │   ├── mod.rs
│       │   ├── settings.rs       # Config struct, XDG paths
│       │   └── secrets.rs        # secret-service keyring integration
│       ├── conversation/
│       │   ├── mod.rs
│       │   ├── manager.rs        # ConversationManager orchestration
│       │   └── title.rs          # Haiku-based title generation
│       └── resources/
│           └── system_prompt.txt # Default system prompt
│
└── clareon-cli/                  # TUI application
    └── src/
        ├── main.rs               # Entry point, CLI argument handling
        ├── cli.rs                # Clap argument definitions
        ├── app.rs                # Application state
        ├── events.rs             # Keyboard event handling
        └── ui/
            └── mod.rs            # ratatui UI components
```

## Building

```bash
# Build
cargo build

# Build release
cargo build --release

# Run tests
cargo test
```

## Running

```bash
# Start TUI with AWS Bedrock (default)
cargo run

# Start with Anthropic API
ANTHROPIC_API_KEY=sk-... cargo run -- --backend anthropic

# List past conversations
cargo run -- --chats

# Resume a conversation
cargo run -- --resume <ID>

# Search conversations
cargo run -- --search "query"

# Start with a specific model
cargo run -- --model anthropic.claude-sonnet-4-20250514-v1:0
```

## TUI Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Enter | Send message |
| Ctrl+N | New conversation |
| Ctrl+O | Open conversation list |
| Ctrl+Q / Ctrl+C | Quit |
| Ctrl+U | Clear input |
| Ctrl+W | Delete word |
| Up/Down | Scroll messages |
| ? / F1 | Show help |

## Configuration

Config file: `~/.config/clareon/config.json`

```json
{
  "default_backend": "bedrock",
  "default_model": "anthropic.claude-sonnet-4-20250514-v1:0",
  "backends": {
    "bedrock": {
      "region": "us-east-1",
      "profile": null
    },
    "anthropic": {
      "api_key_in_keyring": true
    }
  },
  "ui": {
    "theme": "dark",
    "streaming": true
  },
  "system_prompt": {
    "use_default": true,
    "custom_instructions": null
  },
  "models": {
    "title_generation": "anthropic.claude-3-haiku-20240307-v1:0"
  }
}
```

Database: `~/.local/share/clareon/clareon.db`

## Database Schema

```sql
-- Conversations
CREATE TABLE conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    model TEXT NOT NULL,
    system_prompt TEXT,
    custom_instructions TEXT
);

-- Messages with FTS5 search
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id),
    created_at INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    text_content TEXT,        -- For FTS indexing
    content_json TEXT NOT NULL, -- Full structured content
    input_tokens INTEGER,
    output_tokens INTEGER,
    model TEXT
);

CREATE VIRTUAL TABLE messages_fts USING fts5(text_content, content='messages');
```

## Key Dependencies

### clareon-core
- `tokio` - Async runtime
- `sqlx` - SQLite with compile-time checks
- `aws-sdk-bedrockruntime` - AWS Bedrock API
- `reqwest` - HTTP client for Anthropic API
- `secret-service` - D-Bus keyring integration
- `serde` / `serde_json` - Serialization
- `thiserror` - Error types

### clareon-cli
- `ratatui` - TUI framework
- `crossterm` - Terminal backend
- `clap` - CLI argument parsing
- `anyhow` - Application error handling

## Future Roadmap

### Phase 8: Streaming
- Implement streaming in backends
- Real-time response display in TUI

### Phase 9: Basic Tools
- Built-in tools (read_file, write_file, bash)
- Tool approval UI
- Sandboxing with bubblewrap/landlock

### Phase 10: MCP Support
- MCP client (JSON-RPC over stdio)
- Server lifecycle management
- Tool discovery and registration

### Phase 11: QML GUI
- Qt/QML frontend with cxx-qt
- QAbstractItemModel bindings
- Native KDE Plasma integration

### Phase 12: KRunner Integration
- KRunner plugin for quick prompts
- D-Bus integration

### Phase 13: Advanced Features
- Multi-modal (images)
- Conversation summarization
- Context window management
- Export/import conversations

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Async runtime | Tokio | Industry standard, AWS SDK compatibility |
| Database | SQLite + sqlx | Compile-time checks, async, embedded |
| Secrets | secret-service | Pure Rust, D-Bus based, works with GNOME/KDE |
| TUI | ratatui | Popular, flexible, good ecosystem |
| Config format | JSON | Simple, widely supported |
| Message storage | Dual (text + JSON) | FTS on text, full fidelity in JSON |
| IDs | INTEGER | Simpler than UUIDs for local-only app |

## Testing

21 unit tests covering:
- Type serialization/deserialization
- Database CRUD operations
- FTS search
- Configuration loading
- Title generation utilities

Run with: `cargo test`
