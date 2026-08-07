//! Writing one server into `mcp.json`.
//!
//! The body is a *patch*: a field that is absent is left alone. That is what lets the
//! settings page edit a server it was never shown the whole of — `env` and `headers` carry
//! credentials, so their values are never sent to the browser, and a form that overwrote
//! whatever it happened to know would silently delete them.
//!
//! Secrets are therefore edited by name: `envSet` merges, `envRemove` deletes, and
//! anything untouched survives.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::mcp_registry::config::{self, ServerConfig};
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;

use super::mcp_handlers::{commit, validate};

/// A server as the settings page submits it, spelled the same way `mcp.json` spells it.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpsertServer {
    pub r#type: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub auth: Option<String>,
    pub runners: Option<Vec<String>>,
    pub timeout_secs: Option<Option<u32>>,
    /// Variables to add or overwrite. Values never come back out of the API, so this is
    /// how one is changed.
    pub env_set: BTreeMap<String, String>,
    pub env_remove: Vec<String>,
    pub headers_set: BTreeMap<String, String>,
    pub headers_remove: Vec<String>,
}

/// Merge a set/remove pair into a map, removals last so a key in both ends up gone.
fn patch_map(
    target: &mut BTreeMap<String, String>,
    set: BTreeMap<String, String>,
    remove: &[String],
) {
    target.extend(set);
    for key in remove {
        target.remove(key);
    }
}

pub(super) fn apply(
    target: &mut ServerConfig,
    body: UpsertServer,
    builtin: bool,
) -> Result<(), StatusCode> {
    if let Some(runners) = body.runners {
        target.runners = runners
            .iter()
            .map(|raw| crate::runner::RunnerKind::parse(raw).ok_or(StatusCode::UNPROCESSABLE_ENTITY))
            .collect::<Result<Vec<_>, _>>()?;
    }
    if let Some(kind) = body.r#type {
        target.r#type = kind;
    }
    if let Some(command) = body.command {
        target.command = command;
    }
    if let Some(args) = body.args {
        target.args = args;
    }
    if let Some(url) = body.url {
        target.url = url;
    }
    if let Some(auth) = body.auth {
        target.auth = auth;
    }
    if let Some(timeout) = body.timeout_secs {
        target.timeout_secs = timeout;
    }
    patch_map(&mut target.env, body.env_set, &body.env_remove);
    patch_map(&mut target.headers, body.headers_set, &body.headers_remove);

    // A built-in needs no transport of its own: an entry naming one is a patch on opman's
    // definition — turning it off, retiming it, narrowing which runners see it — and
    // demanding a command here would force the user to restate one they never wrote.
    if !builtin && !target.defines_transport() {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

pub async fn upsert_server(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<String>,
    Json(body): Json<UpsertServer>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let name = validate(&name)?;
    let builtin = crate::mcp_registry::builtin::is_builtin(&name);
    let mut document = config::load();
    let entry = document.servers.entry(name.clone()).or_default();
    apply(entry, body, builtin)?;
    commit(&state, &document)?;
    Ok(Json(json!({ "status": "saved", "name": name })))
}

#[cfg(test)]
#[path = "mcp_upsert_tests.rs"]
mod mcp_upsert_tests;
