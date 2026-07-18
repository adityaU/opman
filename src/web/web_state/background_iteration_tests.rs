//! Single-iteration tests for the session/SSE/persist poller bodies extracted
//! from the infinite `spawn_*` loops. Fetch-success branches are driven against
//! a tiny in-process axum mock server; fetch-failure branches use a dead port.

use super::*;
use crate::api::ApiClient;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use std::sync::Arc;

const DEAD_BASE: &str = "http://127.0.0.1:9";

fn mission() -> Mission {
    Mission {
        id: "m1".into(),
        goal: "g".into(),
        session_id: "s1".into(),
        project_index: 0,
        state: MissionState::Pending,
        iteration: 0,
        max_iterations: 5,
        last_verdict: None,
        last_eval_summary: None,
        eval_history: vec![],
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

/// Spin up an axum server that returns canned `/session` and `/session/status`
/// bodies. Returns `(base_url, abort_handle)`.
async fn mock_server(
    sessions: serde_json::Value,
    status: serde_json::Value,
) -> (String, tokio::task::JoinHandle<()>) {
    use axum::routing::get;
    use axum::{Json, Router};

    let s = Arc::new(sessions);
    let st = Arc::new(status);
    let app = Router::new()
        .route(
            "/session",
            get({
                let s = s.clone();
                move || {
                    let s = s.clone();
                    async move { Json((*s).clone()) }
                }
            }),
        )
        .route(
            "/session/status",
            get({
                let st = st.clone();
                move || {
                    let st = st.clone();
                    async move { Json((*st).clone()) }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let h = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), h)
}

// ── session_poll_startup_once ────────────────────────────────────────

#[tokio::test]
async fn startup_once_no_projects_returns_false() {
    let h = WebStateHandle::new_test();
    let client = ApiClient::new();
    assert!(!h.session_poll_startup_once(&client, DEAD_BASE).await);
}

#[tokio::test]
async fn startup_once_dead_base_returns_false_and_no_sessions() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let client = ApiClient::new();
    assert!(!h.session_poll_startup_once(&client, DEAD_BASE).await);
    assert!(h.inner.read().await.projects[0].sessions.is_empty());
}

#[tokio::test]
async fn startup_once_success_hydrates_active_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } },
        { "id": "sx", "title": "x", "directory": "/other", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;
    assert!(h.session_poll_startup_once(&client, &base).await);
    {
        let st = h.inner.read().await;
        // Only the /proj session survives the directory filter.
        assert_eq!(st.projects[0].sessions.len(), 1);
        assert_eq!(st.projects[0].active_session.as_deref(), Some("s1"));
    }
    srv.abort();
}

// ── session_poll_iter_once ───────────────────────────────────────────

#[tokio::test]
async fn iter_once_idle_transition_fires_side_effects() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    // Seed a busy session that the (failing) fetch will not confirm → goes idle.
    h.inner.write().await.busy_sessions.insert("s1".into());
    let mut rx = h.subscribe_events();
    let client = ApiClient::new();
    h.session_poll_iter_once(&client, DEAD_BASE).await;

    assert!(h.inner.read().await.busy_sessions.is_empty());
    let mut saw_idle = false;
    while let Ok(ev) = rx.try_recv() {
        if let WebEvent::SessionIdle { session_id } = ev {
            if session_id == "s1" {
                saw_idle = true;
            }
        }
    }
    assert!(saw_idle, "expected a SessionIdle event for the cleared session");
}

#[tokio::test]
async fn iter_once_busy_and_changed_via_mock() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let mut rx = h.subscribe_events();
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    let status = serde_json::json!({ "s1": { "type": "busy" } });
    let (base, srv) = mock_server(sessions, status).await;
    h.session_poll_iter_once(&client, &base).await;

    assert!(h.inner.read().await.busy_sessions.contains("s1"));
    let (mut saw_busy, mut saw_changed) = (false, false);
    while let Ok(ev) = rx.try_recv() {
        match ev {
            WebEvent::SessionBusy { session_id } if session_id == "s1" => saw_busy = true,
            WebEvent::StateChanged => saw_changed = true,
            _ => {}
        }
    }
    assert!(saw_busy, "expected SessionBusy");
    assert!(saw_changed, "expected StateChanged for the new session list");
    srv.abort();
}

#[tokio::test]
async fn iter_once_no_diff_does_not_emit_changed() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let sess_json =
        serde_json::json!({ "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } });
    // Pre-seed the project's session list to exactly match what the mock returns.
    {
        let si: crate::app::SessionInfo = serde_json::from_value(sess_json.clone()).unwrap();
        h.inner.write().await.projects[0].sessions = vec![si];
    }
    let mut rx = h.subscribe_events();
    let client = ApiClient::new();
    let (base, srv) = mock_server(serde_json::json!([sess_json]), serde_json::json!({})).await;
    h.session_poll_iter_once(&client, &base).await;

    let mut saw_changed = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, WebEvent::StateChanged) {
            saw_changed = true;
        }
    }
    assert!(!saw_changed, "identical session list must not emit StateChanged");
    srv.abort();
}

// ── opencode_sse_reconnect_once ──────────────────────────────────────

#[tokio::test]
async fn reconnect_once_no_projects_is_empty() {
    let h = WebStateHandle::new_test();
    let handles = h.opencode_sse_reconnect_once(DEAD_BASE).await;
    assert!(handles.is_empty());
}

#[tokio::test]
async fn reconnect_once_spawns_one_task_per_project() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    let handles = h.opencode_sse_reconnect_once(DEAD_BASE).await;
    assert_eq!(handles.len(), 2);
    for hh in handles {
        hh.abort();
    }
}

// ── persist_snapshot_once ────────────────────────────────────────────

#[tokio::test]
async fn persist_snapshot_once_writes_to_db() {
    let h = WebStateHandle::new_test();
    h.inner.write().await.missions.insert("m1".into(), mission());
    let res = h.persist_snapshot_once().await;
    assert!(matches!(res, Ok(Ok(()))), "expected a clean persist");
    assert_eq!(h.db_for_test().list_missions().len(), 1);
}
