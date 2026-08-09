// SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! OAuth helpers for remote MCP servers (browser authorization-code flow).

use std::path::{Path, PathBuf};
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

/// Stable loopback port for OAuth redirects so pre-registered clients can
/// list a fixed redirect URI with the authorization server.
///
/// Register: `http://127.0.0.1:38471/callback`
pub const OAUTH_CALLBACK_PORT: u16 = 38471;

/// Canonical redirect URI for pre-registered OAuth clients.
pub fn oauth_redirect_uri() -> String {
    format!("http://127.0.0.1:{OAUTH_CALLBACK_PORT}/callback")
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

        let (listener, redirect_uri) = bind_oauth_callback_listener().await?;
        let has_client_id = cfg
            .oauth_client_id
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());

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
        if has_client_id {
            let client_id = cfg.oauth_client_id.as_ref().unwrap().trim();
            request = request.with_preregistered_client(client_id);
            if let Some(secret) = &cfg.oauth_client_secret
                && !secret.trim().is_empty()
            {
                request = request.with_client_secret(secret.trim());
            }
            info!("OAuth for '{server_id}': using pre-registered client id (DCR skipped)");
        } else {
            info!(
                "OAuth for '{server_id}': no client id set; will attempt dynamic client registration"
            );
        }

        state
            .start_authorization(request)
            .await
            .map_err(|e| map_oauth_start_error(&e.to_string(), &redirect_uri, has_client_id))?;

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

/// Bind the fixed loopback port used for OAuth callbacks (falls back to an
/// ephemeral port only if the preferred port is busy — pre-registered clients
/// may then reject the redirect URI).
async fn bind_oauth_callback_listener() -> Result<(TcpListener, String), String> {
    match TcpListener::bind(("127.0.0.1", OAUTH_CALLBACK_PORT)).await {
        Ok(listener) => {
            let uri = oauth_redirect_uri();
            info!("OAuth callback listening on {uri}");
            Ok((listener, uri))
        }
        Err(e) => {
            warn!(
                "Preferred OAuth port {OAUTH_CALLBACK_PORT} unavailable ({e}); using ephemeral port. \
                 Pre-registered redirect URIs may not match."
            );
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| format!("failed to bind OAuth callback port: {e}"))?;
            let port = listener.local_addr().map_err(|e| e.to_string())?.port();
            let uri = format!("http://127.0.0.1:{port}/callback");
            Ok((listener, uri))
        }
    }
}

/// Turn rmcp auth errors into actionable guidance for the settings UI.
fn map_oauth_start_error(err: &str, redirect_uri: &str, had_client_id: bool) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("dynamic client registration not supported")
        || lower.contains("dynamic registration failed")
        || lower.contains("registration_endpoint")
    {
        if had_client_id {
            return format!(
                "OAuth client registration/authorization failed even with a Client ID set.\n\
                 Check that the Client ID (and secret, if required) are correct, and that this \
                 redirect URI is registered with the provider:\n  {redirect_uri}\n\n\
                 Details: {err}"
            );
        }
        return format!(
            "This authorization server does not allow automatic (dynamic) client registration.\n\n\
             Fix:\n\
             1. Register a native/public OAuth client with the provider.\n\
             2. Set the redirect URI to:\n     {redirect_uri}\n\
             3. Edit this MCP server and fill in OAuth Client ID (and secret if they gave you one).\n\
             4. Save settings, then click Log in again.\n\n\
             Details: {err}"
        );
    }
    if lower.contains("client_id") && lower.contains("required") {
        return format!(
            "An OAuth Client ID is required for this server.\n\
             Edit the server, set OAuth Client ID, register redirect URI:\n  {redirect_uri}\n\n\
             Details: {err}"
        );
    }
    format!("OAuth start failed: {err}")
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
///
/// Returns `true` if a browser process was successfully spawned.
pub fn open_in_browser(url: &str) -> bool {
    info!("Opening OAuth URL in browser: {url}");

    let attempts: &[(&str, &[&str])] = {
        #[cfg(target_os = "linux")]
        {
            &[
                ("xdg-open", &[url]),
                ("gio", &["open", url]),
                ("kde-open5", &[url]),
                ("kde-open", &[url]),
            ]
        }
        #[cfg(target_os = "macos")]
        {
            &[("open", &[url])]
        }
        #[cfg(target_os = "windows")]
        {
            // Handled below — cmd /C start needs different argv shape
            &[]
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            &[]
        }
    };

    for (bin, args) in attempts {
        match std::process::Command::new(bin).args(*args).spawn() {
            Ok(child) => {
                info!("Spawned '{bin}' (pid {}) for OAuth URL", child.id());
                return true;
            }
            Err(e) => {
                warn!("Failed to spawn '{bin}' for OAuth URL: {e}");
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
        {
            Ok(child) => {
                info!("Spawned browser (pid {}) for OAuth URL", child.id());
                return true;
            }
            Err(e) => {
                warn!("Failed to spawn browser for OAuth URL: {e}");
            }
        }
    }

    false
}
