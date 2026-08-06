//! Tests for the metadata routes, focused on where an agent's ACP `mode` slot is routed.
//!
//! ACP calls the slot a mode and leaves the meaning to the agent, so the same field is
//! Claude's permission mode and opencode's agent. These tests pin which picker each one
//! reaches, because getting it wrong offers the user a control that changes nothing.

use std::sync::Arc;

use axum::extract::State;
use serde_json::json;

use super::*;
use crate::acp_engine::config::AgentConfig;
use crate::acp_engine::AcpEngine;

/// An engine whose startup capability probe reported `build`/`plan` in the `mode` option —
/// exactly the `session/new` reply opencode's ACP server sends.
fn engine_with_modes(modes_are_agents: bool) -> Arc<AcpEngine> {
    let agent = AgentConfig {
        display_name: "OpenCode".to_string(),
        command: "opencode".to_string(),
        modes_are_agents,
        ..AgentConfig::default()
    };
    let engine = Arc::new(AcpEngine::new(
        "opencode-acp".to_string(),
        agent,
        None,
        (false, false, false, false),
    ));
    engine.set_capabilities(json!({
        "sessionId": "ses_1",
        "configOptions": [{
            "id": "mode",
            "name": "Session Mode",
            "type": "select",
            "currentValue": "build",
            "options": [
                { "value": "build", "name": "build" },
                { "value": "plan", "name": "plan" },
            ],
        }],
    }));
    engine
}

#[tokio::test]
async fn modes_that_are_agents_are_listed_as_agents() {
    let engine = engine_with_modes(true);
    let Json(agents) = agent_list(State(engine)).await;
    let listed = agents.as_array().expect("array");
    let names: Vec<&str> = listed
        .iter()
        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
        .collect();
    assert_eq!(names, vec!["build", "plan"]);
    // The web UI splits selectable agents from @-mentionable subagents on this field.
    assert_eq!(listed[0]["mode"], "primary");
}

#[tokio::test]
async fn modes_that_are_agents_are_kept_out_of_the_permission_dropdown() {
    let engine = engine_with_modes(true);
    let Json(payload) = provider(State(engine)).await;
    // Empty rather than absent: the client reads absent as "fall back to your own table",
    // which is how opencode's permission list ended up on an unrelated runner.
    assert_eq!(payload["permissionModes"], json!([]));
}

#[tokio::test]
async fn a_permission_mode_agent_still_reports_its_modes() {
    // The default reading of the slot must not change — this is Claude's path.
    let engine = engine_with_modes(false);
    let Json(payload) = provider(State(engine)).await;
    let modes = payload["permissionModes"].as_array().expect("array");
    let values: Vec<&str> = modes
        .iter()
        .filter_map(|m| m.get("value").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(values, vec!["build", "plan"]);

    let Json(agents) = agent_list(State(engine_with_modes(false))).await;
    assert_eq!(agents, json!([]), "ACP itself has no agents to list");
}
