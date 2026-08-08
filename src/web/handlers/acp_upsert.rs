//! Writing one agent into `acp.json`.
//!
//! The body is a patch of a patch: a field that is absent is left as the user's file has
//! it, and a field that is present is written down — including when its value is empty,
//! which is how an argument list or a default mode gets cleared back to nothing.
//!
//! `env` is the exception, edited by name rather than replaced. Its values are usually
//! credentials, so they are never sent to the browser, and a form that overwrote whatever
//! it happened to know would silently delete the ones it never saw.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::acp_engine::config::{self, ClientCaps};
use crate::acp_engine::patch::{self, AgentPatch};
use crate::runner::is_valid_acp_id;
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;

use super::acp_handlers::{commit, outcome, validate_id};

/// An agent as the settings page submits it, spelled the way `acp.json` spells it.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpsertAgent {
    pub display_name: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub runner: Option<String>,
    pub client_caps: Option<ClientCaps>,
    pub inject_mcp: Option<bool>,
    pub default_mode: Option<String>,
    pub default_model: Option<String>,
    pub modes_are_agents: Option<bool>,
    pub subagent_transcripts: Option<bool>,
    pub enabled: Option<bool>,
    /// Inherited variables to strip from the child. Replaced wholesale — these are names,
    /// not secrets, so the page is shown all of them and can send all of them back.
    pub env_remove: Option<Vec<String>>,
    /// Variables to add or overwrite. Values never come back out of the API, so this is
    /// the only way to change one.
    pub env_set: BTreeMap<String, String>,
    /// Variables to delete, applied after `envSet` so a key in both ends up gone.
    pub env_unset: Vec<String>,
}

/// Merge the set/unset pair into the entry's environment, dropping the field again when
/// nothing is left — an empty map in the file would only be noise.
fn patch_env(target: &mut AgentPatch, set: BTreeMap<String, String>, unset: &[String]) {
    if set.is_empty() && unset.is_empty() {
        return;
    }
    let mut env = target.env.take().unwrap_or_default();
    env.extend(set);
    for key in unset {
        env.remove(key);
    }
    target.env = (!env.is_empty()).then_some(env);
}

pub(super) fn apply(target: &mut AgentPatch, body: UpsertAgent) -> Result<(), StatusCode> {
    // A runner slot is a label every persisted session is stored against, so it is held to
    // the same shape as an agent id rather than accepting whatever was typed.
    if let Some(runner) = &body.runner {
        if !runner.is_empty() && !is_valid_acp_id(runner) {
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }
    let UpsertAgent {
        display_name,
        command,
        args,
        runner,
        client_caps,
        inject_mcp,
        default_mode,
        default_model,
        modes_are_agents,
        subagent_transcripts,
        enabled,
        env_remove,
        env_set,
        env_unset,
    } = body;

    // Each `Some` is a decision the user made and is recorded as one; each `None` leaves
    // whatever the entry already said, which may itself be "the built-in decides".
    macro_rules! decide {
        ($($field:ident),+ $(,)?) => {$(
            if $field.is_some() {
                target.$field = $field;
            }
        )+};
    }
    decide!(
        display_name,
        command,
        args,
        runner,
        client_caps,
        inject_mcp,
        default_mode,
        default_model,
        modes_are_agents,
        subagent_transcripts,
        enabled,
        env_remove,
    );
    patch_env(target, env_set, &env_unset);
    Ok(())
}

pub async fn upsert_agent(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<UpsertAgent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = validate_id(&id)?;
    let mut document = patch::load_document();
    apply(document.agents.entry(id.clone()).or_default(), body)?;

    // An agent with nothing to launch can never start, so saving one would leave a row in
    // the list that does nothing and explains nothing. A built-in always has a command,
    // which is why an entry patching one may leave it out.
    let resolved = config::resolve(document.clone());
    let launchable = resolved
        .agents
        .get(&id)
        .is_some_and(|entry| !entry.command.is_empty());
    if !launchable {
        return Err(StatusCode::BAD_REQUEST);
    }

    let changes = commit(&state, &document).await?;
    Ok(outcome("saved", &changes))
}

#[cfg(test)]
#[path = "acp_upsert_tests.rs"]
mod acp_upsert_tests;
