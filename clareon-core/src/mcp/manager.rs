// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! MCP manager: connect servers, cache catalogs, register tools.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, GetPromptRequestParams, Prompt, PromptMessage,
    ReadResourceRequestParams, Resource, Tool as McpTool,
};
use rmcp::service::{Peer, RoleClient, RunningService};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::{McpConfig, McpServerConfig, McpTransportConfig};
use crate::tools::{Tool, ToolRegistry};

use super::content::{flatten_content_blocks, flatten_prompt_messages, flatten_resource_contents};
use super::names::{prefixed_tool_name, unique_name};
use super::tool_adapter::{McpListResourcesTool, McpReadResourceTool, McpToolAdapter};

/// Connection status of a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

/// Snapshot of a server for the UI / status APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatusInfo {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub status: McpServerStatus,
    pub error: Option<String>,
    pub tool_count: usize,
    pub resource_count: usize,
    pub prompt_count: usize,
}

/// Resource reference for UI / meta-tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceRef {
    pub server_id: String,
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Prompt reference for UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptRef {
    pub server_id: String,
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<McpPromptArgRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgRef {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

/// Flattened prompt result ready for conversation injection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptResult {
    pub description: Option<String>,
    pub messages: Vec<McpPromptMessage>,
    /// Human-readable flattened text (all messages).
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptMessage {
    pub role: String,
    pub text: String,
}

struct LiveSession {
    peer: Peer<RoleClient>,
    /// Keep the running service alive for the process / HTTP session.
    _service: RunningService<RoleClient, ()>,
    tools: Vec<McpTool>,
    resources: Vec<Resource>,
    prompts: Vec<Prompt>,
}

struct ServerEntry {
    config: McpServerConfig,
    status: McpServerStatus,
    error: Option<String>,
    live: Option<LiveSession>,
}

/// Owns MCP server sessions and exposes catalogs / tool registration.
pub struct McpManager {
    entries: RwLock<HashMap<String, ServerEntry>>,
    default_timeout: Duration,
}

impl McpManager {
    pub fn new(default_timeout_secs: u64) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            default_timeout: Duration::from_secs(default_timeout_secs.max(1)),
        }
    }

    /// Stop all sessions and connect according to `config`.
    pub async fn reload(&self, config: &McpConfig) {
        // Drop existing sessions first.
        {
            let mut entries = self.entries.write().await;
            for (id, entry) in entries.drain() {
                if let Some(live) = entry.live {
                    // Best-effort cancel; ignore join errors.
                    let _ = live._service.cancel().await;
                    info!("Stopped MCP server '{id}'");
                }
            }
        }

        if !config.enabled {
            info!("MCP is disabled; no servers started");
            return;
        }

        for (id, server_cfg) in &config.servers {
            if !server_cfg.enabled {
                let mut entries = self.entries.write().await;
                entries.insert(
                    id.clone(),
                    ServerEntry {
                        config: server_cfg.clone(),
                        status: McpServerStatus::Disconnected,
                        error: None,
                        live: None,
                    },
                );
                continue;
            }

            {
                let mut entries = self.entries.write().await;
                entries.insert(
                    id.clone(),
                    ServerEntry {
                        config: server_cfg.clone(),
                        status: McpServerStatus::Connecting,
                        error: None,
                        live: None,
                    },
                );
            }

            match connect_server(id, server_cfg).await {
                Ok(live) => {
                    info!(
                        "Connected MCP server '{id}' (tools={}, resources={}, prompts={})",
                        live.tools.len(),
                        live.resources.len(),
                        live.prompts.len()
                    );
                    let mut entries = self.entries.write().await;
                    entries.insert(
                        id.clone(),
                        ServerEntry {
                            config: server_cfg.clone(),
                            status: McpServerStatus::Connected,
                            error: None,
                            live: Some(live),
                        },
                    );
                }
                Err(e) => {
                    warn!("Failed to connect MCP server '{id}': {e}");
                    let mut entries = self.entries.write().await;
                    entries.insert(
                        id.clone(),
                        ServerEntry {
                            config: server_cfg.clone(),
                            status: McpServerStatus::Failed,
                            error: Some(e),
                            live: None,
                        },
                    );
                }
            }
        }
    }

    /// Register MCP tools (and meta-tools) into a registry.
    pub async fn register_tools(self: &Arc<Self>, registry: &mut ToolRegistry) {
        let mut used_names = HashSet::new();
        // Reserve meta-tool names
        used_names.insert("mcp_list_resources".to_string());
        used_names.insert("mcp_read_resource".to_string());

        let entries = self.entries.read().await;
        let mut has_resources = false;

        for (server_id, entry) in entries.iter() {
            let Some(live) = &entry.live else {
                continue;
            };
            if !live.resources.is_empty() {
                has_resources = true;
            }

            let timeout = entry
                .config
                .timeout_secs
                .map(Duration::from_secs)
                .unwrap_or(self.default_timeout);

            for tool in &live.tools {
                let base = prefixed_tool_name(server_id, &tool.name);
                let name = unique_name(base, &mut used_names);
                let description = tool
                    .description
                    .as_ref()
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| format!("MCP tool {} from server {server_id}", tool.name));
                let schema = tool.schema_as_json_value();
                let adapter = McpToolAdapter::new(
                    name,
                    tool.name.to_string(),
                    description,
                    schema,
                    live.peer.clone(),
                    timeout,
                );
                registry.register(Arc::new(adapter) as Arc<dyn Tool>);
            }
        }
        drop(entries);

        if has_resources {
            registry
                .register(Arc::new(McpListResourcesTool::new(Arc::clone(self))) as Arc<dyn Tool>);
            registry
                .register(Arc::new(McpReadResourceTool::new(Arc::clone(self))) as Arc<dyn Tool>);
        }
    }

    pub async fn server_statuses(&self) -> Vec<McpServerStatusInfo> {
        let entries = self.entries.read().await;
        let mut list: Vec<_> = entries
            .iter()
            .map(|(id, e)| {
                let (tool_count, resource_count, prompt_count) = e
                    .live
                    .as_ref()
                    .map(|l| (l.tools.len(), l.resources.len(), l.prompts.len()))
                    .unwrap_or((0, 0, 0));
                McpServerStatusInfo {
                    id: id.clone(),
                    name: e.config.name.clone().unwrap_or_else(|| id.clone()),
                    transport: transport_label(&e.config.transport).to_string(),
                    enabled: e.config.enabled,
                    status: e.status.clone(),
                    error: e.error.clone(),
                    tool_count,
                    resource_count,
                    prompt_count,
                }
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    pub async fn list_resources(&self, server_filter: Option<&str>) -> Vec<McpResourceRef> {
        let entries = self.entries.read().await;
        let mut out = Vec::new();
        for (id, entry) in entries.iter() {
            if let Some(filter) = server_filter
                && filter != id.as_str()
            {
                continue;
            }
            let Some(live) = &entry.live else {
                continue;
            };
            for r in &live.resources {
                out.push(McpResourceRef {
                    server_id: id.clone(),
                    uri: r.uri.clone(),
                    name: r.name.clone(),
                    description: r.description.clone(),
                    mime_type: r.mime_type.clone(),
                });
            }
        }
        out.sort_by(|a, b| (&a.server_id, &a.name).cmp(&(&b.server_id, &b.name)));
        out
    }

    pub async fn read_resource(&self, server_id: &str, uri: &str) -> Result<String, String> {
        let peer = {
            let entries = self.entries.read().await;
            let entry = entries
                .get(server_id)
                .ok_or_else(|| format!("Unknown MCP server '{server_id}'"))?;
            let live = entry
                .live
                .as_ref()
                .ok_or_else(|| format!("MCP server '{server_id}' is not connected"))?;
            live.peer.clone()
        };

        let result = peer
            .read_resource(ReadResourceRequestParams::new(uri.to_string()))
            .await
            .map_err(|e| e.to_string())?;
        Ok(flatten_resource_contents(&result.contents))
    }

    pub async fn list_prompts(&self, server_filter: Option<&str>) -> Vec<McpPromptRef> {
        let entries = self.entries.read().await;
        let mut out = Vec::new();
        for (id, entry) in entries.iter() {
            if let Some(filter) = server_filter
                && filter != id.as_str()
            {
                continue;
            }
            let Some(live) = &entry.live else {
                continue;
            };
            for p in &live.prompts {
                let arguments = p
                    .arguments
                    .as_ref()
                    .map(|args| {
                        args.iter()
                            .map(|a| McpPromptArgRef {
                                name: a.name.clone(),
                                description: a.description.clone(),
                                required: a.required.unwrap_or(false),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(McpPromptRef {
                    server_id: id.clone(),
                    name: p.name.clone(),
                    description: p.description.clone(),
                    arguments,
                });
            }
        }
        out.sort_by(|a, b| (&a.server_id, &a.name).cmp(&(&b.server_id, &b.name)));
        out
    }

    pub async fn get_prompt(
        &self,
        server_id: &str,
        name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<McpPromptResult, String> {
        let peer = {
            let entries = self.entries.read().await;
            let entry = entries
                .get(server_id)
                .ok_or_else(|| format!("Unknown MCP server '{server_id}'"))?;
            let live = entry
                .live
                .as_ref()
                .ok_or_else(|| format!("MCP server '{server_id}' is not connected"))?;
            live.peer.clone()
        };

        let mut params = GetPromptRequestParams::new(name.to_string());
        if let Some(args) = arguments {
            params.arguments = Some(args);
        }

        let result = peer.get_prompt(params).await.map_err(|e| e.to_string())?;
        let messages = result
            .messages
            .iter()
            .map(|m: &PromptMessage| McpPromptMessage {
                role: super::content::prompt_role_label(&m.role).to_string(),
                text: flatten_content_blocks(std::slice::from_ref(&m.content)),
            })
            .collect::<Vec<_>>();
        let text = flatten_prompt_messages(&result.messages);
        Ok(McpPromptResult {
            description: result.description,
            messages,
            text,
        })
    }

    /// Direct tool call (for tests / diagnostics).
    #[allow(dead_code)]
    pub async fn call_tool_raw(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<String, String> {
        let peer = {
            let entries = self.entries.read().await;
            let live = entries
                .get(server_id)
                .and_then(|e| e.live.as_ref())
                .ok_or_else(|| format!("Server '{server_id}' not connected"))?;
            live.peer.clone()
        };
        let mut params = CallToolRequestParams::new(tool_name.to_string());
        if let Some(args) = arguments {
            params = params.with_arguments(args);
        }
        let result = peer.call_tool(params).await.map_err(|e| e.to_string())?;
        Ok(flatten_content_blocks(&result.content))
    }
}

fn transport_label(t: &McpTransportConfig) -> &'static str {
    match t {
        McpTransportConfig::Stdio => "stdio",
        McpTransportConfig::Sse => "sse",
        McpTransportConfig::Http => "http",
    }
}

async fn connect_server(id: &str, cfg: &McpServerConfig) -> Result<LiveSession, String> {
    match cfg.transport {
        McpTransportConfig::Stdio => connect_stdio(id, cfg).await,
        McpTransportConfig::Http | McpTransportConfig::Sse => connect_http(id, cfg).await,
    }
}

async fn connect_stdio(id: &str, cfg: &McpServerConfig) -> Result<LiveSession, String> {
    let command = cfg
        .command
        .as_ref()
        .ok_or_else(|| format!("MCP server '{id}' (stdio) missing command"))?;

    let mut cmd = Command::new(command);
    cmd.args(&cfg.args);
    if let Some(cwd) = &cfg.cwd
        && !cwd.is_empty()
    {
        cmd.current_dir(cwd);
    }
    // Explicit env only (plus PATH/HOME so npx/node work).
    cmd.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    if let Ok(lang) = std::env::var("LANG") {
        cmd.env("LANG", lang);
    }
    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }

    let transport = TokioChildProcess::new(cmd.configure(|_c| {})).map_err(|e| e.to_string())?;

    let service = ().serve(transport).await.map_err(|e| format!("initialize failed: {e}"))?;

    finish_session(service).await
}

async fn connect_http(id: &str, cfg: &McpServerConfig) -> Result<LiveSession, String> {
    let url = cfg
        .url
        .as_ref()
        .ok_or_else(|| format!("MCP server '{id}' (http/sse) missing url"))?;

    let mut headers = HashMap::new();
    for (k, v) in &cfg.headers {
        let name = k
            .parse::<reqwest::header::HeaderName>()
            .map_err(|e| format!("invalid header name '{k}': {e}"))?;
        let value = v
            .parse::<reqwest::header::HeaderValue>()
            .map_err(|e| format!("invalid header value for '{k}': {e}"))?;
        headers.insert(name, value);
    }

    let config =
        StreamableHttpClientTransportConfig::with_uri(url.as_str()).custom_headers(headers);
    let transport = StreamableHttpClientTransport::from_config(config);

    let service = ().serve(transport).await.map_err(|e| format!("initialize failed: {e}"))?;

    finish_session(service).await
}

async fn finish_session(service: RunningService<RoleClient, ()>) -> Result<LiveSession, String> {
    let tools = service
        .list_all_tools()
        .await
        .map_err(|e| format!("list_tools: {e}"))?;
    let resources = service.list_all_resources().await.unwrap_or_else(|e| {
        warn!("list_resources failed (continuing): {e}");
        Vec::new()
    });
    let prompts = service.list_all_prompts().await.unwrap_or_else(|e| {
        warn!("list_prompts failed (continuing): {e}");
        Vec::new()
    });

    let peer = service.peer().clone();
    Ok(LiveSession {
        peer,
        _service: service,
        tools,
        resources,
        prompts,
    })
}
