//! `/api/acp/agents/{id}/native-tools` — switch an agent's own built-in tools on or off.
//!
//! The list endpoint carries the catalogue and the current state (see [`native_tools_view`]);
//! this one writes the choice. It is a one-field patch, like the enable toggle, so nothing
//! else in the agent's entry is touched.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::acp_engine::config::{self, AgentConfig};
use crate::acp_engine::native_tools::NativeTools;
use crate::acp_engine::patch;
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;

use super::acp_handlers::{commit, outcome, validate_id};

/// One tool as the page shows it.
#[derive(Debug, Serialize)]
pub struct ToolView {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

/// The tools opman can switch off for one agent, and which are off. An agent opman has no
/// way to control has no such view at all, so the page cannot offer a dead switch.
#[derive(Debug, Serialize)]
pub struct NativeToolsView {
    pub setting: NativeTools,
    /// Where the switch is applied, in one sentence.
    pub note: &'static str,
    pub tools: Vec<ToolView>,
}

pub fn native_tools_view(entry: &AgentConfig) -> Option<NativeToolsView> {
    let set = entry.tool_set?;
    Some(NativeToolsView {
        setting: entry.native_tools.clone(),
        note: set.mechanism.note(),
        tools: set
            .tools
            .iter()
            .map(|tool| ToolView {
                id: tool.id,
                label: tool.label,
                description: tool.description,
                enabled: !entry.native_tools.turns_off(tool.id),
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
pub struct SetNativeTools {
    pub setting: NativeTools,
}

pub async fn set_native_tools(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<SetNativeTools>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = validate_id(&id)?;
    let mut document = patch::load_document();
    let resolved = config::resolve(document.clone());
    let agent = resolved.agents.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    // An agent with no known mechanism cannot honour the choice, so storing it would only
    // promise something that never happens.
    let set = agent.tool_set.ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
    let setting = body.setting.canonical(set);
    // "All" is the default, so it is stored as silence rather than as a value.
    document.agents.entry(id).or_default().native_tools =
        (setting != NativeTools::All).then_some(setting);
    let changes = commit(&state, document).await?;
    Ok(outcome("saved", &changes))
}

#[cfg(test)]
#[path = "acp_native_tools_tests.rs"]
mod acp_native_tools_tests;
