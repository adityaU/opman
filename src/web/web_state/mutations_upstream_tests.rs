//! Mock-upstream coverage for `mutations.rs`: the `select_session` path that
//! forwards the selection to the opencode server via `base_url()`. The dead-port
//! failure branch is covered elsewhere; here we point `base_url()` at a live mock
//! so the API-call success branch (no warn) executes.
use super::*;
use crate::web::test_support::{scope_base_url, start_mock_upstream};
use crate::web::web_state::WebStateHandle;
use std::path::PathBuf;

fn sess(id: &str, dir: &str) -> crate::app::SessionInfo {
    crate::app::SessionInfo {
        id: id.into(),
        title: format!("title-{id}"),
        parent_id: String::new(),
        directory: dir.into(),
        time: crate::app::SessionTime { created: 1, updated: 2 },
    }
}

#[tokio::test]
async fn select_session_forwards_selection_to_upstream() {
    use axum::routing::post;
    let mock = axum::Router::new().route(
        "/tui/select-session",
        post(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
    );
    let base = start_mock_upstream(mock).await;

    let h = WebStateHandle::new_test_with_projects(vec![("a".into(), PathBuf::from("/a"))]);
    h.add_and_activate_session(0, sess("s1", "/a")).await;

    let mut rx = h.subscribe_events();
    let h2 = h.clone();
    let ok = scope_base_url(base, async move { h2.select_session(0, "s1".into()).await }).await;
    assert!(ok);
    // The successful upstream call still emits StateChanged.
    let mut saw_changed = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, WebEvent::StateChanged) {
            saw_changed = true;
        }
    }
    assert!(saw_changed);
}
