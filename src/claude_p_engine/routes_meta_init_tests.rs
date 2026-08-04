//! Wave-2 coverage for `init_for_dir` cache-miss: a fake `claude` binary on
//! `OPMAN_CLAUDE_BIN` emits a `system/init` line so `introspect` returns real
//! commands/agents, exercising the subprocess path and the cache population.

use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use crate::claude_p_engine::ClaudePEngine;
use axum::http::HeaderValue;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

fn headers(dir: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("x-opencode-directory", HeaderValue::from_str(dir).unwrap());
    h
}

/// Write an executable fake `claude` that prints a canned init line.
fn fake_claude() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake-claude");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' '{\"type\":\"system\",\"subtype\":\"init\",\"slash_commands\":[\"compact\",\"clear\"],\"agents\":[\"Plan\",\"Explore\"]}'\n",
    )
    .unwrap();
    let mut perm = std::fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&script, perm).unwrap();
    dir
}

#[tokio::test]
async fn command_list_cache_miss_introspects_and_caches() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    let bin = fake_claude();
    std::env::set_var("OPMAN_CLAUDE_BIN", bin.path().join("fake-claude"));

    // A real, existing cwd for `introspect`'s `current_dir`.
    let cwd = tempfile::tempdir().unwrap();
    let dir = cwd.path().to_string_lossy().to_string();
    let e = engine();
    assert!(e.cached_init(&dir).is_none(), "starts uncached");

    let Json(v) = command_list(State(e.clone()), headers(&dir)).await;
    let names: Vec<String> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["compact", "clear"]);
    // The introspection result is now cached for the directory.
    let cached = e.cached_init(&dir).expect("cached after first call");
    assert_eq!(
        cached.commands,
        vec!["compact".to_string(), "clear".to_string()]
    );
    assert_eq!(
        cached.agents,
        vec!["Plan".to_string(), "Explore".to_string()]
    );

    match prev {
        Some(p) => std::env::set_var("OPMAN_CLAUDE_BIN", p),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}

#[tokio::test]
async fn agent_list_cache_miss_introspects() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    let bin = fake_claude();
    std::env::set_var("OPMAN_CLAUDE_BIN", bin.path().join("fake-claude"));

    let cwd = tempfile::tempdir().unwrap();
    let dir = cwd.path().to_string_lossy().to_string();
    let e = engine();

    let Json(v) = agent_list(State(e.clone()), headers(&dir)).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Plan");
    assert_eq!(arr[0]["native"], true);
    // Second call hits the cache (introspect not re-run); still returns agents.
    let Json(v2) = agent_list(State(e), headers(&dir)).await;
    assert_eq!(v2.as_array().unwrap().len(), 2);

    match prev {
        Some(p) => std::env::set_var("OPMAN_CLAUDE_BIN", p),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}
