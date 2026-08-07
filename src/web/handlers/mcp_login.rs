//! OAuth login for a proxied MCP server, driven from the settings page.
//!
//! opman usually runs on a remote box behind a tunnel, so the loopback address the
//! authorization server redirects the browser to is unreachable from the user's laptop.
//! The flow is therefore split in two: [`start_login`] hands the authorize URL to the
//! browser, and [`finish_login`] takes the URL the browser ended up at and delivers its
//! query to the loopback listener the flow is already blocked on.
//!
//! The redirect URI is never routed through the tunnel. That would put an authorization
//! code on a public hostname, and most authorization servers reject a non-loopback `http`
//! redirect anyway. Only the *query* of the pasted URL is used — host, port and path
//! always come from opman's own pending flow, so this endpoint cannot be aimed elsewhere.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::mcp_login_state::{callback_query, resolve_env, Pending, Redirect};
use crate::mcp_oauth::{self, flow, OAuthError, ServerName, TokenStore};
use crate::mcp_registry::config::{self, ServerConfig};
use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::types::{ServerState, WebEvent};

/// How long to wait for discovery to produce an authorize URL before giving up. Generous,
/// because it covers up to three metadata fetches plus dynamic client registration.
const DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedLogin {
    /// Open this in a new tab. The page it finally redirects to is on loopback and will
    /// not load for a remote browser — that is expected; the address bar is what matters.
    pub authorize_url: String,
    /// Shown so the user can recognise the address their browser failed to reach.
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishLogin {
    /// The full URL the browser ended up at. A bare query string is accepted too, since
    /// that is what a user copying from the address bar sometimes ends up with.
    pub url: String,
}

/// Begin a login and return the URL the browser must visit.
pub async fn start_login(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(raw): Path<String>,
) -> WebResult<Json<StartedLogin>> {
    let name = ServerName::parse(&raw).map_err(|error| WebError::BadRequest(error.to_string()))?;
    let entry = oauth_entry(&name)?;
    let store = TokenStore::open().map_err(internal)?;

    let (url_tx, mut url_rx) = watch::channel(String::new());
    let http = state.http_client.clone();
    let events = state.event_tx.clone();
    let server = name.clone();
    let mut task: JoinHandle<Result<(), OAuthError>> = tokio::spawn(async move {
        let on_url = move |url: &str| {
            let _ = url_tx.send(url.to_string());
        };
        let record = flow::login(
            &http,
            &server,
            &entry.url,
            &entry.scopes,
            &entry.client_id,
            &resolve_env(&entry.client_secret),
            entry.callback_port.unwrap_or(0),
            &on_url,
        )
        .await?;
        store.save(&server, &record)?;
        // A browser on the same host reaches the loopback listener directly and never
        // calls `finish_login`, so this is the only signal the settings page gets that a
        // credential now exists.
        let _ = events.send(WebEvent::McpServersChanged);
        Ok(())
    });

    // Discovery can fail before any URL is produced, and then the task carries the only
    // useful message — so race the two rather than sitting out the whole timeout.
    let authorize = tokio::select! {
        changed = tokio::time::timeout(DISCOVERY_TIMEOUT, url_rx.changed()) => match changed {
            Ok(Ok(())) => url_rx.borrow().clone(),
            _ => {
                task.abort();
                return Err(fail(&name, "timed out starting the login"));
            }
        },
        joined = &mut task => {
            return Err(match joined {
                Ok(Err(error)) => WebError::BadRequest(error.to_string()),
                _ => fail(&name, "the login flow stopped before it started"),
            });
        }
    };

    let Some(redirect) = Redirect::from_authorize(&authorize) else {
        task.abort();
        return Err(fail(
            &name,
            "the authorization URL carried no loopback redirect",
        ));
    };
    let redirect_uri = redirect.as_str().to_string();
    state
        .mcp_logins
        .arm(name.as_str(), Pending { redirect, task })
        .await;

    Ok(Json(StartedLogin {
        authorize_url: authorize,
        redirect_uri,
    }))
}

/// Hand a pending flow the callback its browser could not deliver.
pub async fn finish_login(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(raw): Path<String>,
    Json(body): Json<FinishLogin>,
) -> WebResult<Json<serde_json::Value>> {
    let name = ServerName::parse(&raw).map_err(|error| WebError::BadRequest(error.to_string()))?;
    let query = callback_query(&body.url)
        .ok_or_else(|| WebError::BadRequest("that does not look like a callback URL".into()))?;
    let pending = state
        .mcp_logins
        .take(name.as_str())
        .await
        .ok_or(WebError::NotFound("no login is waiting for this server"))?;

    // A failure here is the local listener's, not the authorization server's: the flow
    // validates state and issuer itself once the request lands, and reports through its
    // own result — which is what the match below is reading.
    let _ = state
        .http_client
        .get(pending.redirect.delivery(&query))
        .send()
        .await;

    match pending.task.await {
        Ok(Ok(())) => Ok(Json(json!({ "status": "connected" }))),
        Ok(Err(error)) => Err(WebError::BadRequest(error.to_string())),
        Err(_) => Err(fail(&name, "the login flow was cancelled")),
    }
}

/// Forget a credential. Also drops any half-finished flow, so what is left behind matches
/// what the button claims.
pub async fn logout_server(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(raw): Path<String>,
) -> WebResult<Json<serde_json::Value>> {
    let name = ServerName::parse(&raw).map_err(|error| WebError::BadRequest(error.to_string()))?;
    state.mcp_logins.disarm(name.as_str()).await;
    let store = TokenStore::open().map_err(internal)?;
    mcp_oauth::logout(&store, &name).map_err(internal)?;
    let _ = state.event_tx.send(WebEvent::McpServersChanged);
    Ok(Json(json!({ "status": "signed out" })))
}

/// The declared entry for a server that can actually be logged into.
fn oauth_entry(name: &ServerName) -> WebResult<ServerConfig> {
    let mut document = config::load();
    let entry = document
        .servers
        .remove(name.as_str())
        .ok_or(WebError::NotFound("mcp server"))?;
    if entry.auth != "oauth" {
        return Err(WebError::BadRequest(format!(
            "'{name}' does not use OAuth; set its auth to oauth first"
        )));
    }
    if entry.url.is_empty() {
        return Err(WebError::BadRequest(format!(
            "'{name}' has no url, so there is nothing to authorize against"
        )));
    }
    Ok(entry)
}

fn fail(name: &ServerName, detail: &str) -> WebError {
    tracing::warn!(server = name.as_str(), detail, "MCP login failed");
    WebError::BadRequest(detail.to_string())
}

fn internal(error: OAuthError) -> WebError {
    WebError::Internal(error.to_string())
}

#[cfg(test)]
#[path = "mcp_login_tests.rs"]
mod mcp_login_tests;
