// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Import helpers for third-party MCP server configuration formats.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::settings::{McpConfig, McpServerConfig, McpTransportConfig};
use crate::error::{ConfigError, Result};

/// Loose server entry as used by Claude Desktop / Cursor-style configs.
#[derive(Debug, Deserialize)]
struct ExternalServerEntry {
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    /// Alternate name used by some hosts
    #[serde(default, alias = "timeout")]
    timeout: Option<u64>,
}

impl ExternalServerEntry {
    fn into_server_config(self) -> McpServerConfig {
        let transport = match self
            .transport
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("sse") => McpTransportConfig::Sse,
            Some("http") | Some("streamablehttp") | Some("streamable_http") => {
                McpTransportConfig::Http
            }
            Some("stdio") => McpTransportConfig::Stdio,
            _ if self.command.is_some() => McpTransportConfig::Stdio,
            _ if self.url.is_some() => McpTransportConfig::Http,
            _ => McpTransportConfig::Stdio,
        };

        McpServerConfig {
            enabled: self.enabled.unwrap_or(true),
            name: self.name,
            transport,
            command: self.command,
            args: self.args,
            env: self.env,
            cwd: self.cwd,
            url: self.url,
            headers: self.headers,
            bearer_token: None,
            oauth: false,
            oauth_client_id: None,
            oauth_client_secret: None,
            oauth_scopes: Vec::new(),
            timeout_secs: self.timeout_secs.or(self.timeout),
        }
    }
}

/// Parse Claude Desktop / Cursor-style MCP config JSON into an [`McpConfig`].
///
/// Accepts either:
/// - `{ "mcpServers": { "id": { ... } } }`
/// - `{ "servers": { "id": { ... } } }` (Clareon-shaped fragment)
/// - a bare `{ "id": { "command": ... } }` map of servers
pub fn import_mcp_servers_json(json: &str) -> Result<McpConfig> {
    let value: Value = serde_json::from_str(json).map_err(ConfigError::Parse)?;
    let map = extract_server_map(&value)?;

    let mut servers = HashMap::new();
    for (id, entry_value) in map {
        let entry: ExternalServerEntry =
            serde_json::from_value(entry_value).map_err(ConfigError::Parse)?;
        let mut server = entry.into_server_config();
        server.infer_transport();
        servers.insert(id, server);
    }

    Ok(McpConfig {
        enabled: true,
        servers,
    })
}

/// Merge imported servers into an existing config (overwrites same ids).
pub fn merge_imported_servers(target: &mut McpConfig, imported: McpConfig) {
    for (id, server) in imported.servers {
        target.servers.insert(id, server);
    }
    if !target.servers.is_empty() {
        target.enabled = true;
    }
}

fn extract_server_map(value: &Value) -> Result<serde_json::Map<String, Value>> {
    let obj = value
        .as_object()
        .ok_or_else(|| ConfigError::Invalid("MCP import JSON must be an object".into()))?;

    if let Some(servers) = obj.get("mcpServers").and_then(|v| v.as_object()) {
        return Ok(servers.clone());
    }
    if let Some(servers) = obj.get("servers").and_then(|v| v.as_object()) {
        return Ok(servers.clone());
    }

    // Bare map of id → server: each value should be an object (not a bool/string at root).
    let looks_like_servers = obj.values().all(|v| v.is_object());
    if looks_like_servers && !obj.is_empty() {
        return Ok(obj.clone());
    }

    Err(ConfigError::Invalid("No mcpServers/servers map found in MCP import JSON".into()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_claude_desktop_stdio() {
        let json = r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                    "env": { "FOO": "bar" }
                }
            }
        }"#;

        let cfg = import_mcp_servers_json(json).unwrap();
        assert!(cfg.enabled);
        let server = cfg.servers.get("filesystem").unwrap();
        assert_eq!(server.transport, McpTransportConfig::Stdio);
        assert_eq!(server.command.as_deref(), Some("npx"));
        assert_eq!(
            server.args,
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                "/tmp".to_string()
            ]
        );
        assert_eq!(server.env.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn import_url_infers_http() {
        let json = r#"{
            "mcpServers": {
                "remote": {
                    "url": "https://example.com/mcp",
                    "headers": { "Authorization": "Bearer x" }
                }
            }
        }"#;

        let cfg = import_mcp_servers_json(json).unwrap();
        let server = cfg.servers.get("remote").unwrap();
        assert_eq!(server.transport, McpTransportConfig::Http);
        assert_eq!(server.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(
            server.headers.get("Authorization").map(String::as_str),
            Some("Bearer x")
        );
    }

    #[test]
    fn import_explicit_sse() {
        let json = r#"{
            "mcpServers": {
                "legacy": {
                    "transport": "sse",
                    "url": "https://example.com/sse"
                }
            }
        }"#;

        let cfg = import_mcp_servers_json(json).unwrap();
        assert_eq!(
            cfg.servers.get("legacy").unwrap().transport,
            McpTransportConfig::Sse
        );
    }

    #[test]
    fn import_bare_map() {
        let json = r#"{
            "calc": { "command": "mcp-server-calc" }
        }"#;
        let cfg = import_mcp_servers_json(json).unwrap();
        assert!(cfg.servers.contains_key("calc"));
    }

    #[test]
    fn merge_overwrites_ids() {
        let mut target = McpConfig::default();
        target.servers.insert(
            "a".into(),
            McpServerConfig {
                command: Some("old".into()),
                ..Default::default()
            },
        );

        let imported = import_mcp_servers_json(
            r#"{ "mcpServers": { "a": { "command": "new" }, "b": { "command": "b" } } }"#,
        )
        .unwrap();
        merge_imported_servers(&mut target, imported);

        assert_eq!(
            target.servers.get("a").unwrap().command.as_deref(),
            Some("new")
        );
        assert!(target.servers.contains_key("b"));
        assert!(target.enabled);
    }
}
