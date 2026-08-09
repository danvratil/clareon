<!--
SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# MCP support in Clareon

Clareon can connect to [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) servers and expose their **tools**, **resources**, and **prompts** to the assistant.

## Configuration

MCP settings live under the top-level `mcp` key in `~/.config/clareon/config.json` (and in **Settings → Tools & MCP**).

```json
{
  "mcp": {
    "enabled": true,
    "servers": {
      "filesystem": {
        "enabled": true,
        "name": "Filesystem",
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/you/docs"],
        "env": {},
        "cwd": null
      },
      "remote": {
        "enabled": true,
        "transport": "http",
        "url": "https://example.com/mcp",
        "headers": {
          "Authorization": "Bearer …"
        }
      }
    }
  }
}
```

### Transports

| Value   | Meaning |
|---------|---------|
| `stdio` | Spawn a local process; speak MCP over stdin/stdout |
| `http`  | Streamable HTTP remote endpoint |
| `sse`   | Treated like streamable HTTP (legacy label) |

### Remote auth

| Field | Purpose |
|-------|---------|
| `headers` | Map of extra HTTP headers (e.g. `X-Api-Key`) |
| `bearer_token` | Static bearer token (without the `Bearer ` prefix) |
| `oauth` | When `true`, use browser OAuth (authorization code + localhost callback) |
| `oauth_client_id` / `oauth_client_secret` | Optional pre-registered client; empty → dynamic registration |
| `oauth_scopes` | Optional space-separated scopes |

After enabling `oauth` and saving, use **Log in** on the server row. Tokens are stored under `~/.local/share/clareon/mcp_oauth/<server_id>.json` (mode `0600`).

### Import

The settings page accepts Claude Desktop / Cursor-style snippets:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    }
  }
}
```

## Tool naming

MCP tools are registered for the LLM with a stable prefix:

```text
mcp_<server_id>_<tool_name>
```

Non-alphanumeric characters in either segment become `_`. Example: server `file-system` tool `read/file` → `mcp_file_system_read_file`.

Built-in Clareon tools (`read_file`, `write_file`, `list_directory`) keep their unprefixed names.

When any connected server advertises resources, two host meta-tools are also registered:

- `mcp_list_resources` — optional `server` filter  
- `mcp_read_resource` — requires `server` + `uri`

## Resources and prompts

- **Resources**: browse and preview in Settings; the model can list/read via meta-tools.  
- **Prompts**: list in Settings, preview, and optionally inject the flattened text into the current conversation as a user message.

## Security

- Enabling an MCP server is equivalent to running that process or HTTP client **as your user**.
- Stdio servers only receive the environment variables you configure (plus a minimal `PATH` / `HOME` / `LANG` so tools like `npx` work).
- Bubblewrap sandboxing for built-in file tools does **not** isolate MCP child processes.
- Prefer trusted servers; treat remote URLs and auth headers as secrets on disk.

## Reload

Saving settings (or clicking **Reconnect**) rebuilds MCP connections and the tool registry without restarting Clareon.

## Limitations (current)

- Multimodal tool results (images/audio) are summarized as text placeholders.  
- OAuth for remote MCP, sampling, and elicitation are not implemented yet.  
- Servers are global (not per-conversation).
