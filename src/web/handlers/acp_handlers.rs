//! `/api/acp/agents` — read and edit `~/.config/opman/acp.json` from the settings page.
//!
//! Every write follows the same path: mutate the document, persist it, reconcile the live
//! engines against it, and broadcast. Adding an ACP server was already a config edit rather
//! than a code change; this is what makes it not a restart either — the agent becomes a
//! runner in the same request that declared it.
//!
//! Built-ins are handled as they are for MCP servers: opman ships `claude` and `codex`, an
//! entry naming one *patches* opman's definition, and deleting that entry restores the
//! default rather than removing the agent.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::acp_engine::catalog;
use crate::acp_engine::config::{self, AgentConfig, ClientCaps};
use crate::acp_engine::patch::{self, AcpDocument};
use crate::acp_engine::supervisor::AcpChanges;
use crate::runner::{is_valid_acp_id, RunnerKind};
use crate::web::auth::AuthUser;
use crate::web::types::{ServerState, WebEvent};

/// One row in the settings page's agent list: the resolved definition, plus the three facts
/// that are only knowable at runtime — is it live, does opman own it, does its slot clash.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pub id: String,
    pub display_name: String,
    pub command: String,
    pub args: Vec<String>,
    /// Names only. An `env` value is usually a credential, and the page has no use for one
    /// it can only echo back — so the value stays in opman.
    pub env_names: Vec<String>,
    /// Inherited variables stripped from the child, on top of opman's own list.
    pub env_remove: Vec<String>,
    pub runner: String,
    pub client_caps: ClientCaps,
    pub inject_mcp: bool,
    pub default_mode: String,
    pub default_model: String,
    pub modes_are_agents: bool,
    pub subagent_transcripts: bool,
    pub enabled: bool,
    /// opman ships this agent, so removing its entry restores it rather than deleting it.
    pub builtin: bool,
    /// Where opman read this agent's launch command from. Empty for an agent the user
    /// declared themselves; for a catalogue entry opman could not find a documented
    /// command it is the only thing the row can offer, so the page links it.
    pub docs: String,
    /// The user's file has an entry for it.
    pub customized: bool,
    /// An engine is running and the runner slot is served.
    pub running: bool,
    /// Enabled, with a command to launch. A launchable agent that is not running failed to
    /// start, or lost its slot to another engine.
    pub launchable: bool,
    /// Its runner slot is served by an engine the supervisor does not own — `opencode` and
    /// `claude-code` cannot be taken over by an ACP agent.
    pub slot_taken: bool,
    /// This agent serves opman's default runner, so an edit to it lands on the next start
    /// rather than immediately. Its URL went to the TUI once, at startup.
    pub is_default: bool,
}

/// The runtime facts a row needs that the config file cannot answer.
#[derive(Clone, Copy, Default)]
struct Liveness {
    customized: bool,
    running: bool,
    slot_taken: bool,
    is_default: bool,
}

fn view(id: &str, entry: &AgentConfig, live: Liveness) -> AgentView {
    let Liveness {
        customized,
        running,
        slot_taken,
        is_default,
    } = live;
    AgentView {
        id: id.to_string(),
        display_name: entry.display_name.clone(),
        command: entry.command.clone(),
        args: entry.args.clone(),
        env_names: entry.env.keys().cloned().collect(),
        env_remove: entry.env_remove.clone(),
        runner: entry.runner.clone(),
        client_caps: entry.client_caps,
        inject_mcp: entry.inject_mcp,
        default_mode: entry.default_mode.clone(),
        default_model: entry.default_model.clone(),
        modes_are_agents: entry.modes_are_agents,
        subagent_transcripts: entry.subagent_transcripts,
        enabled: entry.enabled,
        builtin: config::is_builtin(id),
        docs: catalog::docs_for(id).unwrap_or_default().to_string(),
        customized,
        running,
        launchable: entry.launchable(),
        slot_taken,
        is_default,
    }
}

/// Every agent opman knows about: the built-ins and anything the user declared.
pub async fn list_agents(
    _auth: AuthUser,
    State(state): State<ServerState>,
) -> Result<Json<Vec<AgentView>>, StatusCode> {
    let document = patch::load_document();
    let resolved = config::resolve(document.clone());
    let running = state.acp.running().await;
    let default = state.acp.default_agent().await;
    let views = resolved
        .agents
        .iter()
        .map(|(id, entry)| {
            let is_running = running.contains_key(id);
            view(
                id,
                entry,
                Liveness {
                    customized: document.agents.contains_key(id),
                    running: is_running,
                    // A slot is only a clash when someone else holds it: an agent already
                    // running holds its own, which conflicts with nothing.
                    slot_taken: !is_running
                        && RunnerKind::parse(&entry.runner)
                            .is_some_and(|kind| state.runner_registry.has(&kind)),
                    is_default: default.as_deref() == Some(id.as_str()),
                },
            )
        })
        .collect();
    Ok(Json(views))
}

/// Persist a change and make the running engines match it.
///
/// The SSE fan-out is the one thing that is registered per runner rather than looked up per
/// request, so a newly installed engine has its stream attached here — without it the agent
/// would answer prompts into a channel no browser is reading.
pub(super) async fn commit(
    state: &ServerState,
    mut document: AcpDocument,
) -> Result<AcpChanges, StatusCode> {
    // An entry that overrides nothing is noise the next reader has to look past, and for a
    // built-in it is also a lie: the row would show as edited for a patch saying nothing.
    document.agents.retain(|_, patch| !patch.is_empty());
    patch::save_document(&document).map_err(|error| {
        tracing::error!("failed to write acp.json: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(adopt(state).await)
}

/// Re-read the file and make the running engines match it, then tell the browsers.
///
/// Split from [`commit`] because the file is not always written on the way in: deleting it
/// is also a change the running set has to follow.
async fn adopt(state: &ServerState) -> AcpChanges {
    let changes = state.acp.reload().await;
    for kind in &changes.added {
        let Some(receiver) = state.runner_registry.event_receiver_for(kind) else {
            continue;
        };
        crate::web::runner_events::spawn_runner_event_receiver(
            receiver,
            kind.display_name().to_string(),
            state.raw_sse_tx.clone(),
            state.web_state.clone(),
        );
    }
    let _ = state.event_tx.send(WebEvent::AcpAgentsChanged);
    changes
}

/// Delete `acp.json` and put every agent back to how opman ships it.
///
/// The per-agent Remove drops one entry; this drops the file. Both mean the same thing —
/// "stop overriding" — but at the two scopes the file actually has, and the whole-file one
/// is the only way to undo a config that has become a mess without knowing what is in it.
pub async fn reset_agents(
    _auth: AuthUser,
    State(state): State<ServerState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let existed = patch::delete_document().map_err(|error| {
        tracing::error!("failed to delete acp.json: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let changes = adopt(&state).await;
    Ok(outcome(
        if existed { "reset" } else { "unchanged" },
        &changes,
    ))
}

/// What a write did, so the page can say "added" or "this slot is taken" rather than
/// silently showing an agent that never started.
pub(super) fn outcome(status: &str, changes: &AcpChanges) -> Json<serde_json::Value> {
    Json(json!({
        "status": status,
        "started": changes.added.iter().map(RunnerKind::display_name).collect::<Vec<_>>(),
        "stopped": changes.removed.iter().map(RunnerKind::display_name).collect::<Vec<_>>(),
        "blocked": changes.blocked,
        "deferred": changes.deferred,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SetEnabled {
    pub enabled: bool,
}

/// Turn an agent on or off. Works for a built-in with no entry of its own: the toggle
/// writes a one-field patch, so opman's launch command is never copied into user config
/// where it would then go stale.
pub async fn set_agent_enabled(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<SetEnabled>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = validate_id(&id)?;
    let mut document = patch::load_document();
    document.agents.entry(id).or_default().enabled = Some(body.enabled);
    let changes = commit(&state, document).await?;
    Ok(outcome("saved", &changes))
}

/// Drop the user's entry. For a built-in that restores opman's own definition; for anything
/// else the agent stops being offered and its processes are killed.
pub async fn delete_agent(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = validate_id(&id)?;
    let mut document = patch::load_document();
    if document.agents.remove(&id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let changes = commit(&state, document).await?;
    Ok(outcome("deleted", &changes))
}

/// Agent ids become runner labels, provider ids and session-file names, so they are held to
/// the same shape the config loader would accept — rejecting one here beats writing it and
/// having the next load ignore it.
pub(super) fn validate_id(raw: &str) -> Result<String, StatusCode> {
    let id = raw.trim();
    if !is_valid_acp_id(id) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(id.to_string())
}

#[cfg(test)]
#[path = "acp_handlers_tests.rs"]
mod acp_handlers_tests;
