# mock-anthropic

A simple HTTP/2 server that implements the Anthropic API for development and testing purposes. It returns Lorem Ipsum text instead of real inference, allowing you to develop and test Anthropic API integration without incurring costs.

## Features

- **Implements core Anthropic API endpoints:**
  - `GET /v1/models` - Lists available Claude models with real model data
  - `POST /v1/messages` - Creates messages with support for both streaming and non-streaming responses

- **Server-Sent Events (SSE) streaming support** - Full streaming response support matching the real Anthropic API
- **API key validation** - Basic authentication to match real API behavior
- **Model validation** - Validates that requested models exist in the model list
- **CORS support** - Allows cross-origin requests for web development

## Running the Server

```bash
# From the workspace root
cargo run -p mock-anthropic

# Or from the mock-anthropic directory
cargo run
```

The server will start on `http://127.0.0.1:8080`.

## Logging

The server uses `tracing` for logging. Control log verbosity with the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run -p mock-anthropic
```

## Usage Examples

### List Models

```bash
curl http://127.0.0.1:8080/v1/models \
  -H 'anthropic-version: 2023-06-01'
```

### Create Message (Non-Streaming)

```bash
curl http://127.0.0.1:8080/v1/messages \
  -H 'Content-Type: application/json' \
  -H 'anthropic-version: 2023-06-01' \
  -H 'x-api-key: your-api-key' \
  -d '{
    "model": "claude-sonnet-4-5-20250929",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello, Claude!"}
    ]
  }'
```

### Create Message (Streaming)

```bash
curl http://127.0.0.1:8080/v1/messages \
  -H 'Content-Type: application/json' \
  -H 'anthropic-version: 2023-06-01' \
  -H 'x-api-key: your-api-key' \
  -d '{
    "model": "claude-sonnet-4-5-20250929",
    "max_tokens": 1024,
    "stream": true,
    "messages": [
      {"role": "user", "content": "Hello, Claude!"}
    ]
  }'
```

## Configuring Clareon to Use Mock Server

To use the mock server with Clareon during development, add the following to your `~/.config/clareon/config.json`:

```json
{
  "default_backend": "anthropic",
  "backends": {
    "anthropic": {
      "api_key_in_keyring": false,
      "base_url": "http://127.0.0.1:8080/v1/messages"
    }
  }
}
```

Then set a dummy API key in your environment (any non-empty value will work):

```bash
export ANTHROPIC_API_KEY="test-key"
```

Start the mock server:

```bash
cargo run -p mock-anthropic
```

Now run Clareon and it will connect to the mock server instead of the real Anthropic API!

## Implementation Notes

- **Model Data**: The server loads real Claude model data from `models-api.json` in the workspace root
- **Response Content**: All message responses return the same Lorem Ipsum text
- **Token Counts**: Input/output token counts are fixed (100 input, 50 output) and don't reflect actual usage
- **Authentication**: Any non-empty API key is accepted
- **Streaming Delay**: Word-by-word streaming has a 50ms delay between chunks to simulate real API behavior

## Development

The mock server is designed to be simple and maintainable. Key implementation details:

- Built with [Axum](https://github.com/tokio-rs/axum) for the HTTP server
- Uses Tokio for async runtime
- Implements SSE streaming using `futures::stream`
- Follows the project's code quality standards (rustfmt + clippy)

## License

GPL-3.0-or-later
