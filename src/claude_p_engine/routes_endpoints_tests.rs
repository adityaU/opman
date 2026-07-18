use super::*;
use crate::claude_p_engine::ClaudePEngine;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

/// Local replica of `test_support::send_json` that also supports the
/// `x-opencode-directory` header the engine's handlers read.
async fn send(
    router: Router,
    method: &str,
    uri: &str,
    dir: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(d) = dir {
        b = b.header("x-opencode-directory", d);
    }
    // Only set a JSON content-type when a body is present, so `Option<Json<_>>`
    // handlers correctly see `None` (defaults) for empty-body requests.
    let req = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, bytes)
}

fn as_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn dir_header_helper() {
    let mut h = HeaderMap::new();
    assert_eq!(dir_header(&h), "");
    h.insert("x-opencode-directory", HeaderValue::from_static("/proj"));
    assert_eq!(dir_header(&h), "/proj");
}

#[tokio::test]
async fn info_and_health() {
    let e = engine();
    let (st, body) = send(router(e.clone()), "GET", "/info", Some("/proj"), None).await;
    assert_eq!(st, StatusCode::OK);
    let v = as_json(&body);
    assert_eq!(v["directory"], "/proj");
    assert!(v["version"].is_string());

    let (st2, body2) = send(router(e), "GET", "/health", None, None).await;
    assert_eq!(st2, StatusCode::OK);
    assert_eq!(&body2, b"ok");
}

#[tokio::test]
async fn list_sessions_filtered_by_dir() {
    let e = engine();
    e.create_session("/proj", "", "one");
    e.create_session("/other", "", "two");
    let (st, body) = send(router(e), "GET", "/session", Some("/proj"), None).await;
    assert_eq!(st, StatusCode::OK);
    let arr = as_json(&body);
    assert_eq!(arr.as_array().unwrap().len(), 1);
    assert_eq!(arr[0]["title"], "one");
    assert_eq!(arr[0]["directory"], "/proj");
}

#[tokio::test]
async fn create_session_with_body_and_defaults() {
    let e = engine();
    let (st, body) =
        send(router(e.clone()), "POST", "/session", Some("/proj"), Some(json!({ "parentID": "p", "title": "T" }))).await;
    assert_eq!(st, StatusCode::OK);
    let v = as_json(&body);
    assert_eq!(v["title"], "T");
    assert_eq!(v["parentID"], "p");
    assert_eq!(v["directory"], "/proj");

    // No body → defaults.
    let (_st, body2) = send(router(e), "POST", "/session", Some("/proj"), None).await;
    let v2 = as_json(&body2);
    assert_eq!(v2["title"], "New session");
    assert_eq!(v2["parentID"], "");
}

#[tokio::test]
async fn get_session_found_and_missing() {
    let e = engine();
    let s = e.create_session("/proj", "", "T");
    let (_st, body) = send(router(e.clone()), "GET", &format!("/session/{}", s.id), None, None).await;
    assert_eq!(as_json(&body)["title"], "T");

    let (_st, body2) = send(router(e), "GET", "/session/missing", None, None).await;
    // Missing → bare `{ id }`.
    assert_eq!(as_json(&body2)["id"], "missing");
}

#[tokio::test]
async fn rename_session_with_and_without_title() {
    let e = engine();
    let s = e.create_session("/proj", "", "T");
    let (_st, body) =
        send(router(e.clone()), "PATCH", &format!("/session/{}", s.id), None, Some(json!({ "title": "New" }))).await;
    assert_eq!(as_json(&body)["title"], "New");

    // Body without title → unchanged, still returns the session.
    let (_st, body2) =
        send(router(e.clone()), "PATCH", &format!("/session/{}", s.id), None, Some(json!({}))).await;
    assert_eq!(as_json(&body2)["title"], "New");

    // Missing session → bare id.
    let (_st, body3) =
        send(router(e), "PATCH", "/session/missing", None, Some(json!({ "title": "x" }))).await;
    assert_eq!(as_json(&body3)["id"], "missing");
}

#[tokio::test]
async fn delete_session_endpoint() {
    let e = engine();
    let s = e.create_session("/proj", "", "T");
    let (st, body) = send(router(e.clone()), "DELETE", &format!("/session/{}", s.id), None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&body)["ok"], true);
    assert!(e.get_session(&s.id).is_none());
}

#[tokio::test]
async fn session_status_lists_busy_only() {
    let e = engine();
    let a = e.create_session("/proj", "", "A");
    let b = e.create_session("/proj", "", "B");
    e.set_busy(&a.id, true);
    let (st, body) = send(router(e), "GET", "/session/status", None, None).await;
    assert_eq!(st, StatusCode::OK);
    let v = as_json(&body);
    assert_eq!(v[&a.id]["type"], "busy");
    assert!(v.get(&b.id).is_none());
}

#[tokio::test]
async fn noop_endpoints() {
    let e = engine();
    for uri in ["/session/x/revert", "/session/x/unrevert", "/tui/select-session"] {
        let (st, body) = send(router(e.clone()), "POST", uri, None, Some(json!({}))).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(as_json(&body)["ok"], true);
    }
    let (st, body) = send(router(e), "POST", "/session/x/share", None, Some(json!({}))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&body), json!({}));
}

#[tokio::test]
async fn provider_endpoint() {
    let e = engine();
    let (st, body) = send(router(e), "GET", "/provider", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&body)["all"][0]["id"], "anthropic");
}

#[tokio::test]
async fn command_and_agent_endpoints_empty_dir() {
    let e = engine();
    let (_s1, b1) = send(router(e.clone()), "GET", "/command", None, None).await;
    assert_eq!(as_json(&b1), json!([]));
    let (_s2, b2) = send(router(e), "GET", "/agent", None, None).await;
    assert_eq!(as_json(&b2), json!([]));
}

#[tokio::test]
async fn command_and_agent_endpoints_cached() {
    let e = engine();
    e.set_cached_init(
        "/proj",
        crate::claude_engine::claude_cli::InitInfo {
            commands: vec!["compact".into()],
            agents: vec!["Plan".into()],
        },
    );
    let (_s1, b1) = send(router(e.clone()), "GET", "/command", Some("/proj"), None).await;
    assert_eq!(as_json(&b1)[0]["name"], "compact");
    let (_s2, b2) = send(router(e), "GET", "/agent", Some("/proj"), None).await;
    assert_eq!(as_json(&b2)[0]["name"], "Plan");
}
