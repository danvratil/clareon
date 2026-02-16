# Clareon

A Claude desktop assistant for Linux with AWS Bedrock and Anthropic API support, featuring a Qt/QML GUI with native desktop integration.

## Project Overview

Clareon is a cross-platform Claude assistant with integrated tool support, conversation management, and native Linux desktop integration. It provides a Qt/QML graphical interface for interacting with Claude models via AWS Bedrock or the Anthropic API, with local conversation storage, full-text search, and built-in file operation tools.

The project includes a terminal UI (clareon-cli) primarily used for testing and prototyping new features.

## Code Style & Licensing

**IMPORTANT:** All source files must begin with SPDX license headers:

```rust
// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later
```

This applies to all `.rs` files in the project. The project is licensed under GPL-3.0-or-later.

### Code Quality Checks

**CRITICAL:** After making any changes to the codebase, you MUST run the following commands and ensure they pass without errors:

```bash
# Format code using rustfmt
cargo fmt --all

# Run clippy with strict linting
cargo clippy --all --locked --tests -- --deny clippy::all --deny warnings
```

**Guidelines:**
- `cargo fmt` must be run on all code changes to ensure consistent formatting
- All clippy warnings and errors must be fixed before committing changes
- Use `#[allow(dead_code)]` for intentionally unused code (e.g., deserialization-only fields)
- Fix actual issues rather than suppressing warnings with `#[allow(...)]` attributes, unless the warning is intentional (e.g., dead code for future use or deserialization)
- Both commands must complete successfully - no errors or warnings are acceptable

## Architecture

The project is organized as a Cargo workspace with four main crates:

- **clareon-core**: Core library containing LLM backends, conversation management, storage, configuration, and tool execution
- **clareon**: Qt/QML GUI application with KDE Plasma integration
- **clareon-cli**: Terminal UI for testing and prototyping (not production-ready)
- **mock-anthropic**: Mock Anthropic API server for development and testing without API costs

### clareon-core Structure

The core library is organized into functional modules:

- `backend/`: LLM backend implementations (Anthropic API, AWS Bedrock) with streaming and prompt caching support
- `config/`: Configuration management, XDG paths, and secret-service keyring integration
- `conversation/`: Conversation management and Haiku-based title generation
- `storage/`: SQLite database with FTS5 full-text search
- `tools/`: Tool execution framework with sandbox support
  - `builtin/`: Built-in tools (read_file, write_file, list_directory)
  - `sandbox/`: Sandboxing implementations (bubblewrap, none)
- `types/`: Core data types (Message, Conversation, ContentBlock, Workspace, etc.)

### clareon Structure

QML GUI components:

- Qt models for conversations and messages
- Application controller bridging Rust and QML
- QML UI components (ChatView, ConversationDrawer, MessageComposer, MessageDelegate)
- Mock data for UI development

## Building

```bash
# Build all crates
cargo build

# Build release
cargo build --release

# Run tests
cargo test

# Build GUI only
cargo build -p clareon
```

### Docker Image

A Docker image with all build dependencies (including Qt libraries) is available at `ghcr.io/danvratil/clareon`. This is intended for building and testing in environments where Qt is not installed, such as Claude Code Web sessions or GitHub Actions workflows. The image is tagged by commit hash and `latest` for the default branch.

```bash
# Pull the latest image
docker pull ghcr.io/danvratil/clareon:latest

# Build inside the container
docker run --rm -v $(pwd):/src -w /src ghcr.io/danvratil/clareon:latest cargo build

# Run tests inside the container
docker run --rm -v $(pwd):/src -w /src ghcr.io/danvratil/clareon:latest cargo test
```

The image is built and published by the `.github/workflows/docker.yml` workflow (triggered manually via `workflow_dispatch`).

## Running

```bash
# Start the QML GUI
cargo run -p clareon

# For testing: Start TUI with AWS Bedrock (default)
cargo run -p clareon-cli

# For testing: Start TUI with Anthropic API
ANTHROPIC_API_KEY=sk-... cargo run -p clareon-cli -- --backend anthropic
```

## Configuration

Config file: `~/.config/clareon/config.json`

Key configuration sections:
- `default_backend`: Choose between "bedrock" or "anthropic"
- `default_model`: Default model to use
- `backends`: Backend-specific settings (AWS region/profile, API keys, prompt caching)
  - `backends.anthropic.base_url`: Custom API endpoint (useful for development/testing with mock server)
  - `backends.anthropic.api_key_in_keyring`: Whether to retrieve API key from system keyring
- `ui`: UI preferences (theme, streaming)
- `system_prompt`: System prompt configuration
- `models`: Model selection for specific tasks (e.g., title generation)
- `tools`: Tool execution configuration (sandbox type, auto-approve)
- `logging`: Logging configuration (level, file output)

Database: `~/.local/share/clareon/clareon.db`

See `clareon-core/src/config/settings.rs` for the full configuration schema.

## Features

### Core Features

- **Multiple Backends**: AWS Bedrock and Anthropic API support
- **Streaming Responses**: Real-time message streaming
- **Prompt Caching**: Automatic caching of system prompts (Claude Sonnet 3.5+, Opus 4, Nova models)
- **Conversation Management**: Create, resume, search, and manage conversations
- **Full-Text Search**: FTS5-based search across all message history
- **Tool Support**: Built-in file operation tools (read, write, list directory)
- **Sandboxing**: Bubblewrap-based sandboxing for tool execution
- **Workspace Management**: Persistent workspace configurations
- **Token Tracking**: Input/output token usage tracking and display
- **Native Qt/QML UI**: Modern interface with KDE Plasma integration

### Database Schema

The database uses SQLite with FTS5 for full-text search:

- `conversations` table: Stores conversation metadata (title, timestamps, model, system prompt)
- `messages` table: Stores messages with dual storage (text for FTS, JSON for full content)
- `messages_fts` virtual table: FTS5 index for fast text search
- `workspaces` table: Stores persistent workspace configurations

See `clareon-core/migrations/` for the complete schema.

## Key Dependencies

### clareon-core
- `tokio`: Async runtime
- `sqlx`: SQLite with compile-time checks and migrations
- `aws-sdk-bedrockruntime`: AWS Bedrock API
- `reqwest`: HTTP client with streaming support
- `secret-service`: D-Bus keyring integration
- `serde`/`serde_json`: Serialization
- `thiserror`: Error types
- `tracing`: Structured logging

### clareon
- `cxx-qt`: Qt/QML integration for Rust
- `cxx-qt-lib`: Qt library bindings

### clareon-cli (testing only)
- `ratatui`: TUI framework
- `crossterm`: Terminal backend
- `clap`: CLI argument parsing

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Async runtime | Tokio | Industry standard, AWS SDK compatibility |
| Database | SQLite + sqlx | Compile-time checks, async, migrations, embedded |
| Secrets | secret-service | Pure Rust, D-Bus based, works with GNOME/KDE |
| GUI | Qt/QML + cxx-qt | Native look, KDE integration, cross-platform |
| Config format | JSON | Simple, widely supported, human-readable |
| Message storage | Dual (text + JSON) | FTS on text, full fidelity in JSON |
| Sandboxing | bubblewrap | Standard Linux tool, namespace isolation |
| Logging | tracing | Structured, async-aware, powerful filtering |

## Testing

Run the test suite with:

```bash
cargo test
```

Tests cover:
- Type serialization/deserialization
- Database CRUD operations and migrations
- FTS search functionality
- Configuration loading and validation
- Backend API interactions
- Tool execution and sandboxing
- Title generation

## Development Notes

- Use `RUST_LOG` environment variable to control logging verbosity (e.g., `RUST_LOG=debug`)
- Tool execution requires explicit workspace configuration for security
- Prompt caching can significantly reduce costs and latency for conversations with long system prompts
- Database migrations are automatically applied on startup
- The TUI (clareon-cli) is primarily for testing and lacks many features present in the GUI
