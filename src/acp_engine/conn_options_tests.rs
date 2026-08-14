//! Pushing a choice onto an agent, against an in-process agent that answers like a real one.

use super::*;

use std::sync::Mutex as StdMutex;

use serde_json::json;

use crate::acp_engine::config::AgentConfig;
use crate::acp_engine::{jsonrpc, options};

/// The reply Claude's adapter gives: the spec's `modes` block *and* a `mode` config option
/// listing the same values. Which of the two an agent actually serves is not knowable from
/// the reply, which is the whole difficulty.
fn setup() -> Value {
    json!({
        "sessionId": "acp-1",
        "modes": {
            "currentModeId": "default",
            "availableModes": [{ "id": "default" }, { "id": "acceptEdits" }],
        },
        "configOptions": [{
            "id": "mode",
            "currentValue": "default",
            "options": [{ "value": "default" }, { "value": "acceptEdits" }],
        }],
    })
}

fn engine() -> (Arc<AcpEngine>, String) {
    let flags = crate::mcp_registry::BuiltinFlags::default();
    let registry = crate::mcp_registry::RegistryHandle::new(
        Arc::new(crate::mcp_registry::McpRegistry::builtins(flags)),
        flags,
    );
    let engine = Arc::new(AcpEngine::new(
        "test".to_string(),
        AgentConfig::default(),
        None,
        registry,
    ));
    let session = engine.create_session("/tmp", "", "options");
    engine.merge_session_setup(&session.id, &setup());
    (engine, session.id)
}

fn calls(log: &Arc<StdMutex<Vec<String>>>) -> Vec<String> {
    log.lock().map(|seen| seen.clone()).unwrap_or_default()
}

/// An agent publishing the spec's `modes` is asked the spec's way, and that is the end of it.
#[tokio::test]
async fn a_spec_mode_is_set_with_set_mode() {
    let (engine, session_id) = engine();
    let log = Arc::new(StdMutex::new(Vec::new()));
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(method.to_string());
        }
        Ok(json!({}))
    });

    push(
        &engine,
        &peer,
        &session_id,
        "acp-1",
        Channel::Mode,
        options::MODE,
        "acceptEdits",
    )
    .await;

    assert_eq!(calls(&log), vec!["session/set_mode"]);
    // The reply carries nothing, so the accepted value has to be written down here — or the
    // next sync compares against a stale current mode and pushes the same one again.
    let stored = engine.session_setup(&session_id);
    assert_eq!(
        options::selected(&stored, options::MODE).as_deref(),
        Some("acceptEdits")
    );
}

/// Some agents publish `modes` without serving `session/set_mode`. Only the agent's own
/// "method not found" can reveal that, so the fallback is a retry rather than a guess — and
/// where the same value is also a config option, that is where the agent really wants it.
#[tokio::test]
async fn a_published_mode_that_is_not_implemented_falls_back_to_the_config_option() {
    let (engine, session_id) = engine();
    let log = Arc::new(StdMutex::new(Vec::new()));
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(method.to_string());
        }
        match method {
            "session/set_mode" => Err(json!({
                "code": jsonrpc::METHOD_NOT_FOUND, "message": "Method not found",
            })),
            _ => Ok(json!({
                "configOptions": [{ "id": "mode", "currentValue": "acceptEdits", "options": [] }],
            })),
        }
    });

    push(
        &engine,
        &peer,
        &session_id,
        "acp-1",
        Channel::Mode,
        options::MODE,
        "acceptEdits",
    )
    .await;

    assert_eq!(
        calls(&log),
        vec!["session/set_mode", "session/set_config_option"]
    );
    let stored = engine.session_setup(&session_id);
    assert_eq!(
        options::current(&stored, options::MODE).as_deref(),
        Some("acceptEdits")
    );
}

/// A refusal that is not "no such method" is the agent's answer, not a wrong guess about how
/// to ask. Retrying it would send the same rejected choice twice.
#[tokio::test]
async fn an_ordinary_refusal_is_not_retried_on_another_channel() {
    let (engine, session_id) = engine();
    let log = Arc::new(StdMutex::new(Vec::new()));
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(method.to_string());
        }
        Err(json!({ "code": -32602, "message": "that mode is not available right now" }))
    });

    push(
        &engine,
        &peer,
        &session_id,
        "acp-1",
        Channel::Mode,
        options::MODE,
        "acceptEdits",
    )
    .await;

    assert_eq!(calls(&log), vec!["session/set_mode"]);
}

/// A failed push must not be recorded as accepted: the session is still in whatever mode the
/// agent was already in, and claiming otherwise would suppress the next attempt.
#[tokio::test]
async fn a_rejected_choice_leaves_the_stored_mode_alone() {
    let (engine, session_id) = engine();
    let peer =
        jsonrpc::fake_agent(|_method, _params| Err(json!({ "code": -32602, "message": "no" })));

    push(
        &engine,
        &peer,
        &session_id,
        "acp-1",
        Channel::Mode,
        options::MODE,
        "acceptEdits",
    )
    .await;

    let stored = engine.session_setup(&session_id);
    assert_eq!(
        options::selected(&stored, options::MODE).as_deref(),
        Some("default")
    );
}

/// `sync` compares before it sends, so a session already on the wanted mode costs no request
/// at all — which is the common case on every turn after the first.
#[tokio::test]
async fn syncing_an_unchanged_choice_sends_nothing() {
    let (engine, session_id) = engine();
    let log = Arc::new(StdMutex::new(Vec::new()));
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, _params| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(method.to_string());
        }
        Ok(json!({}))
    });

    // The agent reported `default`, and that is what the session is set to.
    engine.set_permission_mode(&session_id, "default");
    sync(&engine, &peer, &session_id, "acp-1").await;
    assert!(calls(&log).is_empty(), "{:?}", calls(&log));
}

/// The mode a user picks before their first message reaches the agent on the connection that
/// first message opens. `apply_defaults` is the only chance to say it: `sync` skips a mode
/// the agent already reports, so a mode dropped here is one the whole conversation runs
/// without — the session ran unrestricted while the picker claimed otherwise.
#[tokio::test]
async fn a_new_sessions_chosen_mode_is_pushed_when_its_connection_opens() {
    let (engine, session_id) = engine();
    let log = Arc::new(StdMutex::new(Vec::new()));
    let seen = log.clone();
    let peer = jsonrpc::fake_agent(move |method, params| {
        if let Ok(mut seen) = seen.lock() {
            let mode = params
                .get("modeId")
                .or_else(|| params.get("value"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            seen.push(format!("{method} {mode}"));
        }
        Ok(json!({}))
    });

    engine.set_permission_mode(&session_id, "acceptEdits");
    apply_defaults(&engine, &peer, &session_id, "acp-1", &setup()).await;

    assert_eq!(calls(&log), vec!["session/set_mode acceptEdits"]);
}

/// Opening the connection reads the agent's `session/new` reply, which names the mode the
/// *agent* starts in. Storing that as the session's mode erased the user's pick a moment
/// before `apply_defaults` read it back, so the session ran the agent's mode instead.
#[tokio::test]
async fn the_agents_opening_mode_does_not_replace_a_chosen_one() {
    let (engine, session_id) = engine();
    engine.set_permission_mode(&session_id, "acceptEdits");

    engine.merge_session_setup(&session_id, &setup());

    assert_eq!(engine.effective_mode(&session_id), "acceptEdits");
}

/// A session nobody has configured still takes the agent's word for where it starts — that
/// is the only answer available, and the picker should show what is actually running.
#[tokio::test]
async fn the_agents_opening_mode_fills_an_unconfigured_session() {
    let (engine, session_id) = engine();

    engine.merge_session_setup(&session_id, &setup());

    assert_eq!(engine.effective_mode(&session_id), "default");
}
