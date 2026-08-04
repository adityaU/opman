//! Static/metadata routes for the `claude -p` engine: model provider list, and the
//! per-directory slash-command + agent lists (from claude's `system/init`).

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

use super::routes::{dir_header, Engine};
use crate::claude_engine::claude_cli;

pub(super) async fn provider() -> Json<Value> {
    let model = |id: &str, name: &str| json!({ "id": id, "providerID": "anthropic", "name": name, "limit": { "context": 200_000, "output": 64_000 } });
    Json(json!({
        "all": [{
            "id": "anthropic", "name": "Anthropic",
            "models": {
                "claude-opus-4-8": model("claude-opus-4-8", "Claude Opus 4.8"),
                "claude-sonnet-4-6": model("claude-sonnet-4-6", "Claude Sonnet 4.6"),
                "claude-haiku-4-5-20251001": model("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
            }
        }],
        "connected": ["anthropic"],
        "default": { "anthropic": "claude-sonnet-4-6" },
    }))
}

/// claude's `system/init` introspection (slash commands + agents) for a directory,
/// cached after the first (subprocess) call.
async fn init_for_dir(engine: &Engine, dir: &str) -> claude_cli::InitInfo {
    if let Some(info) = engine.cached_init(dir) {
        return info;
    }
    let d = dir.to_string();
    let info = tokio::task::spawn_blocking(move || claude_cli::introspect(&d))
        .await
        .unwrap_or_default();
    engine.set_cached_init(dir, info.clone());
    info
}

pub(super) async fn command_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    let arr: Vec<Value> = init_for_dir(&engine, &dir)
        .await
        .commands
        .iter()
        .map(|name| json!({ "name": name }))
        .collect();
    Json(Value::Array(arr))
}

pub(super) async fn agent_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    let arr: Vec<Value> = init_for_dir(&engine, &dir)
        .await
        .agents
        .iter()
        .map(|name| json!({ "name": name, "description": "", "mode": "all", "native": true }))
        .collect();
    Json(Value::Array(arr))
}

#[cfg(test)]
#[path = "routes_meta_tests.rs"]
mod routes_meta_tests;

#[cfg(test)]
#[path = "routes_meta_init_tests.rs"]
mod routes_meta_init_tests;
