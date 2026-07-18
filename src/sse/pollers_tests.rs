//! Generated tests for the SSE poller helpers.
//!
//! The `spawn_*` functions loop forever with 3s sleeps and network calls to a
//! dead port, so their loop bodies are not driven here (they'd stall). Instead
//! we (a) invoke each spawner to execute its synchronous setup + first network
//! attempt, and (b) exhaustively test the pure helpers extracted from the loop
//! bodies (`compute_server_active`, `session_transitions`, `max_context_window`).

use super::*;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ── compute_server_active ───────────────────────────────────────────

#[test]
fn compute_server_active_excludes_idle_and_keeps_busy() {
    let mut map = HashMap::new();
    map.insert("a".to_string(), "busy".to_string());
    map.insert("b".to_string(), "retry".to_string());
    map.insert("c".to_string(), "idle".to_string());
    let active = compute_server_active(&map);
    assert_eq!(active, set(&["a", "b"]));
}

#[test]
fn compute_server_active_empty_map_is_empty() {
    let map: HashMap<String, String> = HashMap::new();
    assert!(compute_server_active(&map).is_empty());
}

#[test]
fn compute_server_active_all_idle_is_empty() {
    let mut map = HashMap::new();
    map.insert("x".to_string(), "idle".to_string());
    assert!(compute_server_active(&map).is_empty());
}

// ── session_transitions ─────────────────────────────────────────────

#[test]
fn session_transitions_detects_newly_busy() {
    let server = set(&["a", "b"]);
    let known = set(&["a"]);
    let (busy, idle) = session_transitions(&server, &known);
    assert_eq!(busy, vec!["b".to_string()]);
    assert!(idle.is_empty());
}

#[test]
fn session_transitions_detects_newly_idle() {
    let server = set(&["a"]);
    let known = set(&["a", "z"]);
    let (busy, idle) = session_transitions(&server, &known);
    assert!(busy.is_empty());
    assert_eq!(idle, vec!["z".to_string()]);
}

#[test]
fn session_transitions_no_change() {
    let server = set(&["a", "b"]);
    let known = set(&["a", "b"]);
    let (busy, idle) = session_transitions(&server, &known);
    assert!(busy.is_empty());
    assert!(idle.is_empty());
}

#[test]
fn session_transitions_full_swap() {
    let server = set(&["new"]);
    let known = set(&["old"]);
    let (busy, idle) = session_transitions(&server, &known);
    assert_eq!(busy, vec!["new".to_string()]);
    assert_eq!(idle, vec!["old".to_string()]);
}

#[test]
fn session_transitions_from_empty_known() {
    let server = set(&["a"]);
    let known = HashSet::new();
    let (busy, idle) = session_transitions(&server, &known);
    assert_eq!(busy, vec!["a".to_string()]);
    assert!(idle.is_empty());
}

// ── max_context_window ──────────────────────────────────────────────

#[test]
fn max_context_window_picks_largest() {
    let body = serde_json::json!([
        { "models": {
            "m1": { "limit": { "context": 100_000 } },
            "m2": { "limit": { "context": 400_000 } }
        }},
        { "models": {
            "m3": { "limit": { "context": 200_000 } }
        }}
    ]);
    assert_eq!(max_context_window(&body), 400_000);
}

#[test]
fn max_context_window_non_array_is_zero() {
    assert_eq!(max_context_window(&serde_json::json!({"not": "array"})), 0);
    assert_eq!(max_context_window(&serde_json::Value::Null), 0);
}

#[test]
fn max_context_window_empty_array_is_zero() {
    assert_eq!(max_context_window(&serde_json::json!([])), 0);
}

#[test]
fn max_context_window_missing_models_or_limit_is_zero() {
    // Provider with no models object.
    let body = serde_json::json!([{ "name": "p" }]);
    assert_eq!(max_context_window(&body), 0);
    // Models present but no limit/context.
    let body2 = serde_json::json!([{ "models": { "m": { "id": "m" } } }]);
    assert_eq!(max_context_window(&body2), 0);
    // limit present but context missing.
    let body3 = serde_json::json!([{ "models": { "m": { "limit": {} } } }]);
    assert_eq!(max_context_window(&body3), 0);
}

#[test]
fn max_context_window_non_numeric_context_ignored() {
    let body = serde_json::json!([{ "models": { "m": { "limit": { "context": "big" } } } }]);
    assert_eq!(max_context_window(&body), 0);
}

// ── spawner smoke tests (execute the synchronous setup path) ─────────

#[tokio::test]
async fn spawn_session_poller_does_not_panic() {
    let (tx, _rx) = mpsc::unbounded_channel();
    spawn_session_poller(&tx, 0, "/tmp/some-proj".to_string());
    // The spawned task sleeps 3s before doing anything; we just confirm the
    // spawn call itself is well-formed and returns immediately.
}

#[tokio::test]
async fn spawn_provider_fetcher_runs_first_attempt() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    spawn_provider_fetcher(&tx, 7, "/tmp/some-proj".to_string());
    // Attempt 0 has no leading sleep: it immediately hits the dead base_url,
    // errors, and `continue`s. Give the task a brief moment to start so its
    // closure body is entered. We do not wait for the full 5-attempt fallback.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // Nothing should have been emitted yet (fallback needs ~4 retries of 3s).
    assert!(rx.try_recv().is_err());
}
