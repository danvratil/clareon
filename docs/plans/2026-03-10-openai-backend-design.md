# OpenAI-Compatible Backend Design

**Date:** 2026-03-10
**Status:** Approved

## Goal

Add an OpenAI-compatible backend to Clareon, enabling support for arbitrary models through LiteLLM, OpenRouter, and any other OpenAI-compatible API. This broadens Clareon from an Anthropic-focused assistant to a universal AI assistant.

## Architecture

### New Backend: `OpenAiBackend`

A new backend implementation in `clareon-core/src/backend/openai.rs` using the `openai` crate. It implements the `LlmBackend` trait and maps between Clareon's internal types and the OpenAI chat completions API.

### Config Changes

New `Backend::OpenAi` variant and config section:

```rust
enum Backend { Bedrock, Anthropic, OpenAi }

struct OpenAiConfig {
    api_key: String,           // plaintext in config for now, keyring later
    base_url: Option<String>,  // defaults to https://api.openai.com/v1
}
```

Added as `backends.openai` in `BackendsConfig`. Single endpoint per profile (multi-profile support is a future feature).

### Trait Changes

1. **`available_models()`** becomes async, returns `Result<Vec<ModelInfo>, BackendError>`. Fetches models on-demand from the API rather than returning a hardcoded list. All existing backends updated to wrap their hardcoded lists.

2. **`default_model()`** gets a default implementation. The default model comes from user config, not from the backend.

3. **`StreamEvent`** simplified to not require content block start/stop lifecycle. ContentBlockStart/ContentBlockStop kept but treated as optional — Anthropic emits them, OpenAI backend synthesizes as needed.

### Backend Implementation

- Uses `openai` crate client with configurable base URL
- `send_message()` — maps `ChatRequest` to OpenAI `CreateChatCompletionRequest`, maps response back
- `send_message_stream()` — uses OpenAI SSE streaming, maps chunks to `StreamEvent`
- `available_models()` — calls `/models` endpoint via the `openai` crate
- Tool support from day one — maps `ToolDefinition` to OpenAI function definitions, maps `tool_calls` in responses back to `ContentBlock::ToolUse`

### What Stays The Same

- `ChatRequest`, `ChatResponse`, `Usage`, `StopReason` — no changes needed
- `ContentBlock` types (Text, ToolUse, ToolResult) — already generic enough
- Message/conversation storage — model-agnostic
- `create_backend_from_config()` — just gets a new `Backend::OpenAi` match arm

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| API key storage | Plaintext in config | Simplicity; keyring integration deferred |
| Model listing | Async, on-demand | OpenRouter model lists are very large |
| Default model | From config | User knows their available models |
| Tool support | Day one | Core feature, OpenAI crate supports it |
| StreamEvent | Keep custom, simplify | Separation of concerns from `openai` crate types |
| Multiple endpoints | Single per profile | Multi-profile is a future feature |
