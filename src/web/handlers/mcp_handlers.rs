//! `/api/mcp/servers` — read and edit `~/.config/opman/mcp.json` from the settings page.
//!
//! Every write goes through the same path: mutate the document, persist it, reload the
//! shared registry, and broadcast. Because the registry is swappable and Claude, Codex,
//! and ACP each rebuild their payload per turn or per session, an edit here reaches a
//! live session without restarting anything.
//!
//! The one exception is OpenCode, whose configuration is handed to `opencode serve` once
//! at spawn — that is reported per server so the UI can say so rather than silently
//! doing nothing.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::mcp_registry::config::{self, McpConfig, ServerConfig};
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;

/// One row in the settings page's server list.
///
/// camelCase on the wire, matching `mcp.json`'s own spelling — the page is an editor for
/// that document, so a field should not change name on its way to the browser.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    pub name: String,
    /// `stdio` | `http` | `sse`.
    pub transport: String,
    /// `none` | `static` | `oauth`.
    pub auth: String,
    pub enabled: bool,
    /// True when opman ships this server, so the UI can mark it as not removable.
    pub builtin: bool,
    /// Present for a remote server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Present for a child process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub args: Vec<String>,
    /// Names only. An `env` value is usually an API key, and the settings page has no use
    /// for one it can only echo back — so the value stays in opman.
    pub env_names: Vec<String>,
    /// Names only, for the same reason: a `static` credential lives in a header.
    pub header_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u32>,
    /// Whether a credential is minted by opman rather than living in runner config.
    pub proxied: bool,
    /// Runners this server is withheld from, empty meaning all see it.
    pub runners: Vec<String>,
    /// Requires restarting `opencode serve` to take effect there.
    pub needs_opencode_restart: bool,
    /// Whether a usable credential is stored. Only meaningful when `auth` is `oauth`;
    /// the UI reads it to decide between "Connected" and "Needs login".
    pub authenticated: bool,
}

fn view(name: &str, entry: &ServerConfig, builtin: bool, authenticated: bool) -> ServerView {
    let remote = entry.remote_kind();
    ServerView {
        name: name.to_string(),
        transport: match remote {
            Some(crate::mcp_registry::spec::RemoteKind::Http) => "http",
            Some(crate::mcp_registry::spec::RemoteKind::Sse) => "sse",
            None => "stdio",
        }
        .to_string(),
        auth: if entry.auth.is_empty() {
            "none".to_string()
        } else {
            entry.auth.clone()
        },
        enabled: entry.enabled,
        builtin,
        url: (!entry.url.is_empty()).then(|| entry.url.clone()),
        command: (!entry.command.is_empty()).then(|| entry.command.clone()),
        args: entry.args.clone(),
        env_names: entry.env.keys().cloned().collect(),
        header_names: entry.headers.keys().cloned().collect(),
        timeout_secs: entry.timeout_secs,
        proxied: remote.is_some() && entry.auth != "none" && !entry.auth.is_empty(),
        runners: entry.runners.iter().map(|r| r.to_string()).collect(),
        needs_opencode_restart: true,
        authenticated,
    }
}

/// Whether `name` has a usable OAuth credential right now.
///
/// The store is opened per listing rather than held in state: it is a directory of files
/// any `opman mcp-proxy` child may have refreshed since, so a cached view of it would go
/// stale in exactly the case the settings page is asked about.
fn authenticated(store: Option<&crate::mcp_oauth::TokenStore>, name: &str) -> bool {
    let Some(store) = store else { return false };
    crate::mcp_oauth::ServerName::parse(name)
        .map(|parsed| crate::mcp_oauth::is_authenticated(store, &parsed))
        .unwrap_or(false)
}

/// The declared servers, plus the built-ins the user has not written an entry for.
pub async fn list_servers(
    _auth: AuthUser,
    State(state): State<ServerState>,
) -> Result<Json<Vec<ServerView>>, StatusCode> {
    let declared = config::load();
    let registry = state.mcp.current();
    let store = crate::mcp_oauth::TokenStore::open().ok();
    let mut views: Vec<ServerView> = Vec::new();
    for (name, entry) in &declared.servers {
        let builtin = crate::mcp_registry::builtin::is_builtin(name);
        let signed_in = entry.auth == "oauth" && authenticated(store.as_ref(), name);
        views.push(view(name, entry, builtin, signed_in));
    }
    // Built-ins with no entry of their own still belong in the list, so they can be
    // toggled without the user first having to write a stub.
    for spec in registry.all() {
        if declared.servers.contains_key(spec.name()) {
            continue;
        }
        views.push(ServerView {
            name: spec.name().to_string(),
            transport: "stdio".to_string(),
            auth: "none".to_string(),
            enabled: true,
            builtin: crate::mcp_registry::builtin::is_builtin(spec.name()),
            url: None,
            command: None,
            args: Vec::new(),
            env_names: Vec::new(),
            header_names: Vec::new(),
            timeout_secs: None,
            proxied: false,
            runners: Vec::new(),
            needs_opencode_restart: true,
            authenticated: false,
        });
    }
    views.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(views))
}

/// Persist a change and make it live for every runner that rebuilds per turn.
pub(super) fn commit(state: &ServerState, document: &McpConfig) -> Result<(), StatusCode> {
    config::save(document).map_err(|error| {
        tracing::error!("failed to write mcp.json: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    state.mcp.reload();
    let _ = state.event_tx.send(crate::web::types::WebEvent::McpServersChanged);
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SetEnabled {
    pub enabled: bool,
}

/// Turn a server on or off. Works for a built-in with no entry of its own: the toggle
/// writes a patch rather than a full definition, so opman's own launch command is never
/// duplicated into user config.
pub async fn set_enabled(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<String>,
    Json(body): Json<SetEnabled>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let name = validate(&name)?;
    let mut document = config::load();
    document.servers.entry(name.clone()).or_default().enabled = body.enabled;
    commit(&state, &document)?;
    Ok(Json(json!({ "status": "saved", "enabled": body.enabled })))
}

pub async fn delete_server(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let name = validate(&name)?;
    let mut document = config::load();
    if document.servers.remove(&name).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    commit(&state, &document)?;
    Ok(Json(json!({ "status": "deleted" })))
}

/// Server names become JSON object keys and proxy arguments, so keep them to the same
/// shape a skill name uses rather than accepting anything.
pub(super) fn validate(raw: &str) -> Result<String, StatusCode> {
    let name = raw.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(name.to_string())
}

#[cfg(test)]
#[path = "mcp_handlers_tests.rs"]
mod mcp_handlers_tests;
