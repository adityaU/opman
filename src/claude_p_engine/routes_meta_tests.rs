use super::*;
use crate::claude_engine::claude_cli::InitInfo;
use crate::claude_p_engine::ClaudePEngine;
use axum::http::HeaderValue;
use std::sync::Arc;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

fn headers(dir: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    if !dir.is_empty() {
        h.insert("x-opencode-directory", HeaderValue::from_str(dir).unwrap());
    }
    h
}

#[tokio::test]
async fn provider_serves_discovered_models() {
    let e = engine();
    // Seed the startup cache so the route never shells out to the CLI here.
    e.set_cached_models(crate::claude_engine::models::default_models());

    let Json(v) = provider(State(e)).await;
    assert_eq!(v["all"][0]["id"], "anthropic");
    assert!(v["all"][0]["models"]["claude-opus-5"].is_object());
    assert_eq!(
        v["all"][0]["models"]["claude-opus-5"]["limit"]["context"],
        1_000_000
    );
    assert_eq!(v["connected"][0], "anthropic");
    assert_eq!(v["default"]["anthropic"], "claude-fable-5");
}

#[tokio::test]
async fn provider_prefers_cached_models_over_fallback() {
    let e = engine();
    e.set_cached_models(vec![crate::claude_engine::claude_cli::ModelInfo {
        id: "claude-opus-9".into(),
        display_name: "Opus 9".into(),
        context_window: 42,
        max_output: 7,
    }]);
    let Json(v) = provider(State(e)).await;
    assert_eq!(v["all"][0]["models"]["claude-opus-9"]["limit"]["output"], 7);
    assert_eq!(v["default"]["anthropic"], "claude-opus-9");
}

#[tokio::test]
async fn command_list_empty_dir_is_empty() {
    let e = engine();
    let Json(v) = command_list(State(e), headers("")).await;
    assert_eq!(v, Value::Array(vec![]));
}

#[tokio::test]
async fn command_list_from_cached_init() {
    let e = engine();
    e.set_cached_init(
        "d1",
        InitInfo {
            commands: vec!["compact".into(), "clear".into()],
            agents: vec![],
        },
    );
    let Json(v) = command_list(State(e), headers("d1")).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "compact");
    assert_eq!(arr[1]["name"], "clear");
}

#[tokio::test]
async fn agent_list_empty_dir_is_empty() {
    let e = engine();
    let Json(v) = agent_list(State(e), headers("")).await;
    assert_eq!(v, Value::Array(vec![]));
}

#[tokio::test]
async fn agent_list_from_cached_init() {
    let e = engine();
    e.set_cached_init(
        "d1",
        InitInfo {
            commands: vec![],
            agents: vec!["Plan".into(), "Explore".into()],
        },
    );
    let Json(v) = agent_list(State(e), headers("d1")).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Plan");
    assert_eq!(arr[0]["native"], true);
    assert_eq!(arr[0]["mode"], "all");
}
