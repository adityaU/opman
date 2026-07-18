//! Generated tests for `context_handlers.rs`.
//!
//! `get_session_todos` and the provider fetch in `get_context_window` proxy to
//! the opencode server (unreachable → the provider fetch falls back to the
//! default limit). `update_session_todos` writes to `$HOME/.local/share/
//! opencode/opencode.db`, so HOME is redirected to a temp dir. The pure
//! `build_context_window_response` helper is exercised directly.

use super::*;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::{ServerState, WebSessionStats};
use axum::http::StatusCode;
use serde_json::json;

fn init_base_url() {
    let _ = crate::app::BASE_URL.set("http://127.0.0.1:1/".to_string());
}

fn isolate_env() {
    use std::sync::OnceLock;
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_CONFIG_HOME", d.path());
        std::env::set_var("XDG_STATE_HOME", d.path());
        d
    });
}

async fn state_with_project() -> (ServerState, tempfile::TempDir) {
    isolate_env();
    init_base_url();
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = test_server_state();
    state
        .web_state
        .add_project(tmp.path().to_str().unwrap(), None)
        .await
        .expect("add project");
    (state, tmp)
}

/// Serialises HOME mutation across the todo-writing tests.
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as HOME_LOCK;

// ── get_session_todos ───────────────────────────────────────────────

#[tokio::test]
async fn get_todos_no_project_bad_request() {
    let state = test_server_state();
    let (status, _) =
        send_json(test_router(state), "GET", "/api/session/s1/todos", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_todos_upstream_error() {
    let (state, _tmp) = state_with_project().await;
    let (status, _) =
        send_json(test_router(state), "GET", "/api/session/s1/todos", None).await;
    assert!(status.is_server_error(), "got {status}");
}

// ── get_context_window ──────────────────────────────────────────────

#[tokio::test]
async fn context_window_no_project_bad_request() {
    let state = test_server_state();
    let (status, _) =
        send_json(test_router(state), "GET", "/api/context-window?session_id=s1", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn context_window_with_session_id_falls_back_to_default_limit() {
    let (state, _tmp) = state_with_project().await;
    let (status, body) = send_json(
        test_router(state),
        "GET",
        "/api/context-window?session_id=s1",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // Providers fetch fails (no upstream) → 200_000 fallback, zero stats.
    assert_eq!(v["context_limit"], 200_000);
    assert_eq!(v["total_used"], 0);
    assert_eq!(v["categories"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn context_window_no_active_session_bad_request() {
    // Project present but no active session and no session_id query param.
    let (state, _tmp) = state_with_project().await;
    let (status, _) =
        send_json(test_router(state), "GET", "/api/context-window", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── update_session_todos ────────────────────────────────────────────

#[tokio::test]
async fn update_todos_success_empty() {
    let home = tempfile::tempdir().unwrap();
    let ocdir = home.path().join(".local/share/opencode");
    std::fs::create_dir_all(&ocdir).unwrap();
    let conn = rusqlite::Connection::open(ocdir.join("opencode.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE todo (session_id TEXT, content TEXT, status TEXT, priority TEXT, \
         position INTEGER, time_created INTEGER, time_updated INTEGER);",
    )
    .unwrap();
    drop(conn);

    let state = test_server_state();
    let _guard = HOME_LOCK.lock().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", home.path());
    let (status, body) = send_json(
        test_router(state),
        "PUT",
        "/api/session/s1/todos",
        Some(json!([])),
    )
    .await;
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn update_todos_success_with_item() {
    let home = tempfile::tempdir().unwrap();
    let ocdir = home.path().join(".local/share/opencode");
    std::fs::create_dir_all(&ocdir).unwrap();
    let conn = rusqlite::Connection::open(ocdir.join("opencode.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE todo (session_id TEXT, content TEXT, status TEXT, priority TEXT, \
         position INTEGER, time_created INTEGER, time_updated INTEGER);",
    )
    .unwrap();
    drop(conn);

    let state = test_server_state();
    let _guard = HOME_LOCK.lock().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", home.path());
    let (status, body) = send_json(
        test_router(state),
        "PUT",
        "/api/session/s1/todos",
        Some(json!([{ "content": "do it", "status": "pending", "priority": "high" }])),
    )
    .await;
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["content"], "do it");
}

#[tokio::test]
async fn update_todos_db_open_error() {
    // HOME with no opencode dir → Connection::open fails → 500.
    let home = tempfile::tempdir().unwrap();
    let state = test_server_state();
    let _guard = HOME_LOCK.lock().unwrap();
    let old = std::env::var_os("HOME");
    std::env::set_var("HOME", home.path());
    let (status, _) = send_json(
        test_router(state),
        "PUT",
        "/api/session/s1/todos",
        Some(json!([])),
    )
    .await;
    match old {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    assert!(status.is_server_error(), "got {status}");
}

// ── build_context_window_response (pure) ────────────────────────────

#[test]
fn build_ctx_all_categories() {
    let stats = WebSessionStats {
        input_tokens: 100,
        output_tokens: 50,
        reasoning_tokens: 10,
        cache_read: 5,
        cache_write: 5,
        ..Default::default()
    };
    let r = build_context_window_response(&stats, 1000);
    assert_eq!(r.context_limit, 1000);
    assert_eq!(r.total_used, 170);
    assert!((r.usage_pct - 17.0).abs() < 1e-9);
    // input, output, reasoning, cache
    assert_eq!(r.categories.len(), 4);
    let names: Vec<&str> = r.categories.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["input", "output", "reasoning", "cache"]);
    // cache category holds both read + write items.
    let cache = r.categories.iter().find(|c| c.name == "cache").unwrap();
    assert_eq!(cache.tokens, 10);
    assert_eq!(cache.items.len(), 2);
    // estimated = remaining(830) / (total_used/2 = 85) = 9
    assert_eq!(r.estimated_messages_remaining, Some(9));
}

#[test]
fn build_ctx_zero_stats() {
    let stats = WebSessionStats::default();
    let r = build_context_window_response(&stats, 1000);
    assert_eq!(r.total_used, 0);
    assert_eq!(r.usage_pct, 0.0);
    assert!(r.categories.is_empty());
    assert_eq!(r.estimated_messages_remaining, None);
}

#[test]
fn build_ctx_zero_limit() {
    let stats = WebSessionStats { input_tokens: 100, ..Default::default() };
    let r = build_context_window_response(&stats, 0);
    assert_eq!(r.total_used, 100);
    assert_eq!(r.usage_pct, 0.0);
    // one category (input), pct 0 due to zero limit
    assert_eq!(r.categories.len(), 1);
    assert_eq!(r.categories[0].pct, 0.0);
    // context_limit (0) not > total_used → None
    assert_eq!(r.estimated_messages_remaining, None);
}

#[test]
fn build_ctx_cache_read_only() {
    let stats = WebSessionStats { cache_read: 20, ..Default::default() };
    let r = build_context_window_response(&stats, 1000);
    let cache = r.categories.iter().find(|c| c.name == "cache").unwrap();
    assert_eq!(cache.items.len(), 1);
    assert_eq!(cache.items[0].label, "Cache Read");
}

#[test]
fn build_ctx_cache_write_only() {
    let stats = WebSessionStats { cache_write: 20, ..Default::default() };
    let r = build_context_window_response(&stats, 1000);
    let cache = r.categories.iter().find(|c| c.name == "cache").unwrap();
    assert_eq!(cache.items.len(), 1);
    assert_eq!(cache.items[0].label, "Cache Write");
}

#[test]
fn build_ctx_estimate_uses_default_when_no_input() {
    // No input tokens but output present → avg falls back to 10_000.
    let stats = WebSessionStats { output_tokens: 100, ..Default::default() };
    let r = build_context_window_response(&stats, 1000);
    // remaining = 900, avg = 10_000 → 0
    assert_eq!(r.estimated_messages_remaining, Some(0));
}
