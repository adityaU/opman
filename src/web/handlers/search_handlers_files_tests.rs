//! Generated coverage tests for `search_handlers.rs`.

use super::*;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use crate::web::auth::AuthUser;
use crate::web::test_support::test_server_state;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

fn state_dir(p: &std::path::Path) -> ServerState {
    let mut s = test_server_state();
    s.web_state = WebStateHandle::new_test_with_projects(vec![("t".into(), p.to_path_buf())]);
    s
}

fn auth() -> AuthUser {
    AuthUser { subject: "t".into() }
}

async fn body_json<T: IntoResponse>(r: Result<T, WebError>) -> (axum::http::StatusCode, serde_json::Value) {
    let resp = r.into_response();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

// ── default_search_limit ───────────────────────────────────────────

#[test]
fn default_search_limit_is_50() {
    assert_eq!(default_search_limit(), 50);
}

// ── build_snippet ──────────────────────────────────────────────────

#[test]
fn build_snippet_no_match_truncates() {
    let s = build_snippet("hello world", "zzz", 4);
    assert_eq!(s, "hell");
}

#[test]
fn build_snippet_match_at_start() {
    let s = build_snippet("needle in haystack here", "needle", 10);
    assert!(s.starts_with("needle"));
    assert!(!s.starts_with("..."));
    assert!(s.ends_with("..."));
}

#[test]
fn build_snippet_match_in_middle() {
    let hay = "aaaaaaaaaaaaaaaaaaaa needle bbbbbbbbbbbbbbbbbbbb";
    let s = build_snippet(hay, "needle", 10);
    assert!(s.starts_with("..."));
    assert!(s.ends_with("..."));
    assert!(s.contains("needle"));
}

#[test]
fn build_snippet_match_at_end() {
    let s = build_snippet("some text then needle", "needle", 8);
    assert!(s.starts_with("..."));
    assert!(s.contains("needle"));
    assert!(!s.ends_with("..."));
}

#[test]
fn build_snippet_short_haystack_no_ellipsis() {
    let s = build_snippet("needle", "needle", 100);
    assert_eq!(s, "needle");
}

#[test]
fn build_snippet_unicode_boundaries() {
    let hay = "café ☕ needle 日本語テキスト";
    let s = build_snippet(hay, "needle", 12);
    assert!(s.contains("needle"));
    // must be valid UTF-8 (String guarantees it) and not panic
    assert!(!s.is_empty());
}

// ── get_file_edits ─────────────────────────────────────────────────

#[tokio::test]
async fn get_file_edits_empty() {
    let state = test_server_state();
    let (st, v) = body_json(get_file_edits(State(state), auth(), Path("sess-x".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["file_count"], 0);
    assert_eq!(v["session_id"], "sess-x");
}

#[tokio::test]
async fn get_file_edits_dedups_by_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fpath = tmp.path().join("edited.txt");
    std::fs::write(&fpath, "v1").unwrap();
    let state = state_dir(tmp.path());
    // two edits to the same file → deduped to one entry (latest)
    state.web_state.record_file_edit("s1", "edited.txt", Some(tmp.path())).await;
    std::fs::write(&fpath, "v2").unwrap();
    state.web_state.record_file_edit("s1", "edited.txt", Some(tmp.path())).await;

    let (st, v) = body_json(get_file_edits(State(state), auth(), Path("s1".to_string())).await).await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["file_count"], 1);
    assert_eq!(v["edits"].as_array().unwrap().len(), 1);
    assert_eq!(v["edits"][0]["path"], "edited.txt");
}

// ── search_messages ────────────────────────────────────────────────

#[tokio::test]
async fn search_messages_empty_query() {
    let state = test_server_state();
    let (st, v) = body_json(
        search_messages(
            State(state),
            auth(),
            Path(0usize),
            Query(SearchQuery { q: "   ".into(), limit: 50 }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["total"], 0);
}

#[tokio::test]
async fn search_messages_invalid_project_400() {
    let state = test_server_state();
    let (st, _) = body_json(
        search_messages(
            State(state),
            auth(),
            Path(99usize),
            Query(SearchQuery { q: "hello".into(), limit: 50 }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_messages_valid_project_no_sessions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = state_dir(tmp.path());
    let (st, v) = body_json(
        search_messages(
            State(state),
            auth(),
            Path(0usize),
            Query(SearchQuery { q: "hello".into(), limit: 500 }),
        )
        .await,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    assert_eq!(v["total"], 0);
    assert_eq!(v["query"], "hello");
}
