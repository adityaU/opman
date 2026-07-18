//! Coverage tests (wave 3) for `search_handlers.rs` — the `search_messages`
//! opencode-proxy SUCCESS path. Prior waves only reached the `Err(_) => continue`
//! arm because `base_url()` pointed at a dead port. Here we stand up a mock
//! upstream via `start_mock_upstream` + `scope_base_url` so the real fetch
//! succeeds, `resp.json()` parses, and `collect_session_matches` runs inside the
//! handler loop — exercising the response-mapping bulk of the handler.

use super::*;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use crate::web::auth::AuthUser;
use crate::web::test_support::{scope_base_url, start_mock_upstream, test_server_state};
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("proj".into(), p.to_path_buf())]);
    s
}

fn auth() -> AuthUser {
    AuthUser { subject: "t".into() }
}

fn sess(id: &str, dir: &str) -> crate::app::SessionInfo {
    crate::app::SessionInfo {
        id: id.into(),
        title: format!("title-{id}"),
        parent_id: String::new(),
        directory: dir.into(),
        time: crate::app::SessionTime { created: 1, updated: 2 },
    }
}

async fn body_json<T: IntoResponse>(
    r: Result<T, WebError>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let resp = r.into_response();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

// ── SUCCESS: upstream returns matching messages → results populated ──

#[tokio::test]
async fn search_messages_success_returns_matches() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(serde_json::json!([
                {
                    "info": { "role": "assistant", "id": "m1", "time": { "created": 99 } },
                    "parts": [ { "text": "here is the NEEDLE we search for" } ]
                },
                {
                    "info": { "role": "user", "id": "m2", "time": { "created": 100 } },
                    "parts": [ { "text": "totally unrelated content" } ]
                }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;

    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    state
        .web_state
        .add_and_activate_session(0, sess("s1", tmp.path().to_str().unwrap()))
        .await;

    let (st, v) = scope_base_url(base, async move {
        body_json(
            search_messages(
                State(state),
                auth(),
                Path(0usize),
                Query(SearchQuery { q: "needle".into(), limit: 50 }),
            )
            .await,
        )
        .await
    })
    .await;

    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["total"], 1);
    assert_eq!(v["query"], "needle");
    let hit = &v["results"][0];
    assert_eq!(hit["message_id"], "m1");
    assert_eq!(hit["role"], "assistant");
    assert_eq!(hit["session_id"], "s1");
    assert_eq!(hit["project_name"], "proj");
    assert_eq!(hit["timestamp"], 99);
    assert!(hit["snippet"].as_str().unwrap().to_lowercase().contains("needle"));
}

// ── upstream returns non-JSON body → resp.json() Err → continue ──────

#[tokio::test]
async fn search_messages_upstream_invalid_json_continues() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async { "this-is-not-json{{{".to_string() }),
    );
    let base = start_mock_upstream(mock).await;

    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    state
        .web_state
        .add_and_activate_session(0, sess("s1", tmp.path().to_str().unwrap()))
        .await;

    let (st, v) = scope_base_url(base, async move {
        body_json(
            search_messages(
                State(state),
                auth(),
                Path(0usize),
                Query(SearchQuery { q: "needle".into(), limit: 50 }),
            )
            .await,
        )
        .await
    })
    .await;

    assert_eq!(st, axum::http::StatusCode::OK);
    // JSON parse failed for the only session → no matches collected.
    assert_eq!(v["total"], 0);
}

// ── limit reached → outer `results.len() >= limit` break skips later sessions ─

#[tokio::test]
async fn search_messages_cross_session_limit_break() {
    let mock = axum::Router::new().route(
        "/session/{id}/message",
        get(|| async {
            axum::Json(serde_json::json!([
                { "info": { "role": "user", "id": "x", "time": { "created": 1 } },
                  "parts": [ { "text": "match needle here" } ] }
            ]))
        }),
    );
    let base = start_mock_upstream(mock).await;

    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    // Two sessions, but limit=1 → after session 1 fills the single slot, the
    // outer loop breaks before fetching session 2.
    state
        .web_state
        .add_and_activate_session(0, sess("s1", tmp.path().to_str().unwrap()))
        .await;
    state
        .web_state
        .add_and_activate_session(0, sess("s2", tmp.path().to_str().unwrap()))
        .await;

    let (st, v) = scope_base_url(base, async move {
        body_json(
            search_messages(
                State(state),
                auth(),
                Path(0usize),
                Query(SearchQuery { q: "needle".into(), limit: 1 }),
            )
            .await,
        )
        .await
    })
    .await;

    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["total"], 1);
}
