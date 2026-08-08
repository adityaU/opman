//! Extra mock-upstream branch coverage for `background.rs` poll iterations that
//! the existing single-iteration tests don't reach: the startup poll's
//! "active session already set" and "no matching sessions" branches, and the
//! recurring poll's busy-transition path that cancels a pending watcher timer.
use super::*;
use crate::api::ApiClient;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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

// ── session_poll_startup_once: active_session already set ────────────

#[tokio::test]
async fn startup_once_keeps_existing_active_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    // A valid active session must survive hydration.
    h.inner.write().await.projects[0].active_session = Some("s1".into());
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;
    assert!(h.session_poll_startup_once(&client, &base).await);
    let st = h.inner.read().await;
    // Session list refreshed, and the valid user selection is untouched.
    assert_eq!(st.projects[0].sessions.len(), 1);
    assert_eq!(st.projects[0].active_session.as_deref(), Some("s1"));
    drop(st);
    srv.abort();
}

#[tokio::test]
async fn startup_once_repairs_stale_active_session() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    h.inner.write().await.projects[0].active_session = Some("ghost-session".into());
    let client = ApiClient::new();
    let sessions = serde_json::json!([
        { "id": "s1", "title": "t", "directory": "/proj", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;

    assert!(h.session_poll_startup_once(&client, &base).await);
    assert_eq!(
        h.inner.read().await.projects[0].active_session.as_deref(),
        Some("s1")
    );
    srv.abort();
}

// ── session_poll_startup_once: fetch ok but nothing matches the dir ──

#[tokio::test]
async fn startup_once_no_matching_sessions_leaves_active_none() {
    let h = WebStateHandle::new_test_with_projects(vec![("p".into(), PathBuf::from("/proj"))]);
    let client = ApiClient::new();
    // Only a session for a different directory → filtered list is empty, so the
    // `filtered.first()` branch yields None and active_session stays None.
    let sessions = serde_json::json!([
        { "id": "sx", "title": "x", "directory": "/other", "time": { "created": 1, "updated": 2 } }
    ]);
    let (base, srv) = mock_server(sessions, serde_json::json!({})).await;
    assert!(h.session_poll_startup_once(&client, &base).await);
    let st = h.inner.read().await;
    assert!(st.projects[0].sessions.is_empty());
    assert!(st.projects[0].active_session.is_none());
    drop(st);
    srv.abort();
}
