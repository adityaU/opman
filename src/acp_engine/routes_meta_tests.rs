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
        crate::mcp_registry::RegistryHandle::default(),
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

/// With lazy start, the catalogue read is what boots the runner, so it lands while the
/// startup probe is still out. It must wait for the probe rather than answer empty — the
/// picker never asks twice.
#[tokio::test]
async fn provider_waits_for_an_inflight_probe() {
    let engine = Arc::new(AcpEngine::new(
        "claude".to_string(),
        AgentConfig::default(),
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ));
    engine.probing.send_replace(true);
    let probe = engine.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        probe.set_capabilities(json!({
            "models": {
                "currentModelId": "opus",
                "availableModels": [{ "modelId": "opus", "name": "Opus" }],
            },
        }));
        probe.probing.send_replace(false);
    });
    let Json(payload) = provider(State(engine)).await;
    assert!(payload["all"][0]["models"].get("opus").is_some(), "{payload}");
}

/// An engine that never probed answers at once, empty, instead of waiting out the timeout.
#[tokio::test(start_paused = true)]
async fn provider_without_a_probe_does_not_wait() {
    let engine = Arc::new(AcpEngine::new(
        "claude".to_string(),
        AgentConfig::default(),
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ));
    let started = tokio::time::Instant::now();
    let Json(payload) = provider(State(engine)).await;
    assert_eq!(started.elapsed(), std::time::Duration::ZERO);
    assert_eq!(payload["all"][0]["models"], json!({}));
}
