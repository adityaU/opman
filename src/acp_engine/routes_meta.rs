//! Metadata routes: model catalog, slash commands, agent list.
//!
//! All three are answered from what the agent reported over ACP rather than from a table
//! in opman. Before any session exists there is nothing to report, and the honest answer is
//! an empty list — the picker fills in as soon as the first session negotiates.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

use super::options;
use super::routes::{dir_header, Engine};

/// Model catalog in the opencode `/provider` shape.
pub(super) async fn provider(State(engine): State<Engine>) -> Json<Value> {
    let current = engine.current_model();
    // An agent whose modes are really its agents has no permission model reachable over
    // ACP — its permissions live in its own config. Reporting an empty list says exactly
    // that, and the picker hides the dropdown rather than offering choices nothing reads.
    let modes = if engine.agent.modes_are_agents {
        Vec::new()
    } else {
        engine.modes()
    };
    Json(options::provider_payload(
        &engine.id,
        &engine.agent.display_name,
        &engine.models(),
        current.as_deref(),
        &modes,
    ))
}

/// Slash commands the agent advertised for this directory.
pub(super) async fn command_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    let commands: Vec<Value> = engine
        .commands_for_dir(&dir)
        .iter()
        .filter_map(|c| {
            let name = c.get("name").and_then(Value::as_str)?;
            let mut command = json!({
                "name": name,
                "description": c.get("description").and_then(Value::as_str).unwrap_or(""),
            });
            // ACP's optional `input.hint` is the agent's own word for what it expects after
            // the command — "<pattern>", "[file]". Carried through as `args` so the picker
            // shows the agent's phrasing rather than opman guessing at an argument shape.
            let hint = c
                .get("input")
                .and_then(|input| input.get("hint"))
                .and_then(Value::as_str)
                .filter(|hint| !hint.is_empty());
            if let (Some(hint), Some(object)) = (hint, command.as_object_mut()) {
                object.insert("args".to_string(), Value::String(hint.to_string()));
            }
            Some(command)
        })
        .collect();
    Json(Value::Array(commands))
}

/// Selectable agents, in the opencode `/agent` shape.
///
/// ACP has no agent concept of its own, so this is empty for most agents and the picker
/// falls back to a single default entry. The exception is an agent that fills the `mode`
/// slot with its own agents rather than with permission modes: those belong here, where
/// they are picked from the list, and not in the permission dropdown.
pub(super) async fn agent_list(State(engine): State<Engine>) -> Json<Value> {
    if !engine.agent.modes_are_agents {
        return Json(Value::Array(vec![]));
    }
    let agents: Vec<Value> = engine
        .modes()
        .iter()
        .map(|mode| {
            json!({
                "name": mode.id,
                "description": mode.description,
                // opencode's own field for "can be chosen for the session", as opposed to a
                // subagent that may only be @-mentioned.
                "mode": "primary",
                "native": true,
            })
        })
        .collect();
    Json(Value::Array(agents))
}

#[cfg(test)]
#[path = "routes_meta_tests.rs"]
mod routes_meta_tests;
