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

#### Pre-registered clients (most hosts)

Many authorization servers **do not** support OAuth Dynamic Client Registration. In that case Clareon cannot invent a client id for you:

1. Register a **native / public** OAuth application with the provider.
2. Set the redirect URI exactly to:
   ```
   http://127.0.0.1:38471/callback
   ```
3. Put the issued **Client ID** (and secret only if they gave you one) in the MCP server settings.
4. Save, then **Log in**.

If Client ID is left empty, Clareon tries dynamic registration and will fail with a clear error when the server rejects it.

#### GitHub remote MCP (`https://api.githubcopilot.com/mcp/`)

GitHub **explicitly does not support Dynamic Client Registration** for the remote MCP
([host integration guide](https://github.com/github/github-mcp-server/blob/main/docs/host-integration.md),
[issue #1404](https://github.com/github/github-mcp-server/issues/1404)). That is a GitHub product
choice, not a Clareon bug. VS Code “just works” because it ships a **pre-registered** GitHub OAuth
client inside the editor — third-party hosts do not get to reuse that client.

**Recommended for Clareon: PAT**

```json
{
  "mcp": {
    "servers": {
      "github": {
        "enabled": true,
        "transport": "http",
        "url": "https://api.githubcopilot.com/mcp/",
        "oauth": false,
        "bearer_token": "github_pat_…"
      }
    }
  }
}
```

Or via the UI: HTTP transport, OAuth **off**, Bearer token = your PAT, then Reconnect.

**OAuth path:** create your own [GitHub App](https://docs.github.com/en/apps/creating-github-apps) or
[OAuth App](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/creating-an-oauth-app),
callback `http://127.0.0.1:38471/callback`, put client id/secret on the server, Log in.

**Local stdio alternative:** run `github-mcp-server` / Docker with GitHub’s baked-in OAuth app
(see their [oauth-login.md](https://github.com/github/github-mcp-server/blob/main/docs/oauth-login.md)).

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
