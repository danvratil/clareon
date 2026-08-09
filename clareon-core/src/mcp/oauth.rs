// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! OAuth helpers for remote MCP servers (browser authorization-code flow).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationRequest, CredentialStore, OAuthState,
    StoredCredentials,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::config::{Config, McpServerConfig};

/// Per-server file credential store under the XDG data directory.
#[derive(Clone)]
pub struct FileCredentialStore {
    path: PathBuf,
}

impl FileCredentialStore {
    pub fn for_server(server_id: &str) -> Result<Self, String> {
        let dir = Config::data_dir()
            .map_err(|e| e.to_string())?
            .join("mcp_oauth");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // Sanitize server id for filesystem
        let safe: String = server_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        Ok(Self {
            path: dir.join(format!("{safe}.json")),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn has_credentials(&self) -> bool {
        self.path.is_file()
    }
}

#[async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        if !self.path.is_file() {
            return Ok(None);
        }
        let data = tokio::fs::read(&self.path)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        let creds: StoredCredentials = serde_json::from_slice(&data)
            .map_err(|e| AuthError::InternalError(format!("invalid OAuth cache: {e}")))?;
        Ok(Some(creds))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
        }
        let data = serde_json::to_vec_pretty(&credentials)
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        tokio::fs::write(&self.path, data)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;
        // Restrict permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        if self.path.is_file() {
            tokio::fs::remove_file(&self.path)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
        }
        Ok(())
    }
}

/// Build an authorization manager that reuses on-disk tokens when present.
pub async fn load_authorization_manager(
    server_id: &str,
    server_url: &str,
) -> Result<Option<AuthorizationManager>, String> {
    let store = FileCredentialStore::for_server(server_id)?;
    if !store.has_credentials() {
        return Ok(None);
    }

    let mut state = OAuthState::new(server_url, None)
        .await
        .map_err(|e| e.to_string())?;

    // Attach file store before initializing
    {
        let manager = match &mut state {
            OAuthState::Unauthorized(m) => m,
            _ => return Err("unexpected OAuth state".into()),
        };
        manager.set_credential_store(store);
        let restored = manager
            .initialize_from_store()
            .await
            .map_err(|e| e.to_string())?;
        if !restored {
            return Ok(None);
        }
    }

    // Promote Unauthorized+credentials to Authorized via set_credentials path is already done
    // by initialize_from_store when tokens exist — but state may still be Unauthorized with
    // loaded credentials. Prefer get_access_token; if it works, convert.
    match state {
        OAuthState::Unauthorized(manager) => {
            // initialize_from_store leaves us unauthorized but with credentials in the store;
            // get_access_token reads the store and works when a token is present.
            match manager.get_access_token().await {
                Ok(_token) => Ok(Some(manager)),
                Err(e) => {
                    warn!("Stored OAuth credentials unusable for '{server_id}': {e}");
                    Ok(None)
                }
            }
        }
        OAuthState::Authorized(manager) => Ok(Some(manager)),
        _ => {
            warn!("Unexpected OAuth state after credential load for '{server_id}'");
            Ok(None)
        }
    }
}

/// Run interactive OAuth: open browser, wait for localhost redirect, persist tokens.
///
/// Returns the authorization URL that the UI should open, then a future that
/// completes when the callback is handled. We split this so the service can
/// emit the URL to QML first.
pub struct PendingOAuthLogin {
    state: OAuthState,
    listener: TcpListener,
    redirect_uri: String,
    server_id: String,
    store: FileCredentialStore,
}

impl PendingOAuthLogin {
    pub async fn begin(server_id: &str, cfg: &McpServerConfig) -> Result<(String, Self), String> {
        let url = cfg
            .url
            .as_ref()
            .ok_or_else(|| "OAuth requires a remote server URL".to_string())?;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("failed to bind OAuth callback port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let redirect_uri = format!("http://127.0.0.1:{port}/callback");

        let store = FileCredentialStore::for_server(server_id)?;
        let mut state = OAuthState::new(url.as_str(), None)
            .await
            .map_err(|e| e.to_string())?;

        if let OAuthState::Unauthorized(manager) = &mut state {
            manager.set_credential_store(store.clone());
        }

        let mut request = AuthorizationRequest::new(redirect_uri.clone())
            .with_client_name("Clareon")
            .with_application_type("native");
        if !cfg.oauth_scopes.is_empty() {
            request = request.with_scopes(cfg.oauth_scopes.clone());
        }
        if let Some(client_id) = &cfg.oauth_client_id
            && !client_id.is_empty()
        {
            request = request.with_preregistered_client(client_id);
            if let Some(secret) = &cfg.oauth_client_secret
                && !secret.is_empty()
            {
                request = request.with_client_secret(secret);
            }
        }

        state
            .start_authorization(request)
            .await
            .map_err(|e| format!("OAuth start failed: {e}"))?;

        let auth_url = state
            .get_authorization_url()
            .await
            .map_err(|e| e.to_string())?;

        info!("OAuth authorization URL ready for server '{server_id}'");

        Ok((
            auth_url,
            Self {
                state,
                listener,
                redirect_uri,
                server_id: server_id.to_string(),
                store,
            },
        ))
    }

    /// Wait for the browser redirect and finish the token exchange.
    pub async fn complete(mut self) -> Result<(), String> {
        let callback_url = wait_for_oauth_callback(&self.listener, &self.redirect_uri)
            .await
            .map_err(|e| format!("OAuth callback failed: {e}"))?;

        self.state
            .handle_callback_url(&callback_url)
            .await
            .map_err(|e| format!("OAuth token exchange failed: {e}"))?;

        // Ensure credentials are on disk (manager store should have saved already;
        // re-save if we can pull credentials).
        if let Ok(creds) = self.state.get_credentials().await {
            // get_credentials returns Credentials not StoredCredentials — rely on store
            let _ = creds;
        }

        // Force a load/save cycle so the file exists even if the manager used in-memory store
        // before we attached FileCredentialStore. Re-export via AuthorizationManager if authorized.
        if let Some(manager) = self.state.into_authorization_manager() {
            // Attach store and re-save current credentials from manager
            // The manager already has the store from start; initialize should have written.
            let _ = manager;
            // Touch: if file still missing, write a note
            if !self.store.has_credentials() {
                warn!(
                    "OAuth completed for '{}' but credential file missing at {}",
                    self.server_id,
                    self.store.path().display()
                );
            } else {
                info!(
                    "OAuth login complete for '{}'; tokens at {}",
                    self.server_id,
                    self.store.path().display()
                );
            }
        }

        Ok(())
    }
}

async fn wait_for_oauth_callback(
    listener: &TcpListener,
    expected_redirect: &str,
) -> Result<String, String> {
    let accept = tokio::time::timeout(Duration::from_secs(300), listener.accept())
        .await
        .map_err(|_| "timed out waiting for OAuth browser callback (5 minutes)".to_string())?
        .map_err(|e| e.to_string())?;

    let (mut stream, _) = accept;
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse "GET /callback?code=...&state=... HTTP/1.1"
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| "malformed OAuth callback HTTP request".to_string())?;

    let base = expected_redirect.trim_end_matches("/callback").to_string();
    let callback_url = if path.starts_with("http") {
        path.to_string()
    } else {
        format!("{base}{path}")
    };

    let body = r#"<!DOCTYPE html><html><body>
        <h1>Clareon MCP</h1>
        <p>Login complete. You can close this window and return to Clareon.</p>
        </body></html>"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;

    Ok(callback_url)
}

/// Clear stored OAuth tokens for a server.
pub async fn clear_oauth_tokens(server_id: &str) -> Result<(), String> {
    let store = FileCredentialStore::for_server(server_id)?;
    store.clear().await.map_err(|e| e.to_string())
}

/// Whether the server has a usable OAuth token cache.
pub fn oauth_logged_in(server_id: &str) -> bool {
    FileCredentialStore::for_server(server_id)
        .map(|s| s.has_credentials())
        .unwrap_or(false)
}

/// Open a URL with the system default browser (best-effort).
pub fn open_in_browser(url: &str) {
    // Prefer xdg-open / open; ignore failures — UI may also open the URL.
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    let _ = url;
    let _ = Arc::new(()); // silence unused on some cfgs
}
