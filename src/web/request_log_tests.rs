//! Tests for `request_log.rs`.
//!
//! The middleware itself is exercised end-to-end through a tiny router (status
//! codes, cancellation) while `classify`/`body_bytes` are tested directly.

use super::*;
use axum::body::Body;
use axum::http::{Request as HttpRequest, StatusCode};
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;

// ── classify ────────────────────────────────────────────────────────

#[test]
fn classify_plain_api_route() {
    assert_eq!(classify("/api/state"), Traffic::Api);
    assert_eq!(classify("/api/session/abc/message"), Traffic::Api);
}

#[test]
fn classify_health_is_api() {
    assert_eq!(classify("/health"), Traffic::Api);
}

#[test]
fn classify_internal_route_is_api() {
    assert_eq!(classify("/internal/kanban/task/t1/status"), Traffic::Api);
}

#[test]
fn classify_sse_streams() {
    assert_eq!(classify("/api/events"), Traffic::Stream);
    assert_eq!(classify("/api/session/events"), Traffic::Stream);
    assert_eq!(classify("/api/editor/events"), Traffic::Stream);
    assert_eq!(classify("/api/pty/stream"), Traffic::Stream);
    assert_eq!(classify("/api/system/stats/stream"), Traffic::Stream);
}

#[test]
fn classify_websocket_upgrade() {
    assert_eq!(classify("/api/mcp/ws"), Traffic::Stream);
}

#[test]
fn classify_frontend_paths_are_assets() {
    assert_eq!(classify("/"), Traffic::Asset);
    assert_eq!(classify("/assets/index-a1b2.js"), Traffic::Asset);
    assert_eq!(classify("/favicon.ico"), Traffic::Asset);
    // A path merely *containing* /api/ is still the SPA fallback.
    assert_eq!(classify("/docs/api/overview"), Traffic::Asset);
}

#[test]
fn classify_stream_suffix_only_applies_under_api() {
    // The suffix rule must not promote a static file to Stream.
    assert_eq!(classify("/vendor/events"), Traffic::Asset);
}

// ── body_bytes ──────────────────────────────────────────────────────

#[test]
fn body_bytes_reads_content_length() {
    let mut h = HeaderMap::new();
    h.insert(axum::http::header::CONTENT_LENGTH, "512".parse().unwrap());
    assert_eq!(body_bytes(&h), 512);
}

#[test]
fn body_bytes_missing_header_is_zero() {
    assert_eq!(body_bytes(&HeaderMap::new()), 0);
}

#[test]
fn body_bytes_unparseable_header_is_zero() {
    let mut h = HeaderMap::new();
    h.insert(axum::http::header::CONTENT_LENGTH, "abc".parse().unwrap());
    assert_eq!(body_bytes(&h), 0);
}

// ── middleware ──────────────────────────────────────────────────────

fn router() -> Router {
    Router::new()
        .route("/api/ok", get(|| async { "hi" }))
        .route(
            "/api/boom",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "no") }),
        )
        .layer(axum::middleware::from_fn(log_requests))
}

#[tokio::test]
async fn middleware_passes_response_through_unchanged() {
    let res = router()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/ok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 64).await.unwrap();
    assert_eq!(&body[..], b"hi");
}

#[tokio::test]
async fn middleware_preserves_error_status() {
    let res = router()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/boom")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn middleware_preserves_404_from_fallback() {
    let res = router()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Dropping the in-flight future must not panic — this is the cancellation
/// path the drop guard reports on.
#[tokio::test]
async fn dropping_inflight_request_is_clean() {
    let slow = Router::new()
        .route(
            "/api/slow",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                "done"
            }),
        )
        .layer(axum::middleware::from_fn(log_requests));

    let fut = slow.oneshot(
        HttpRequest::builder()
            .uri("/api/slow")
            .body(Body::empty())
            .unwrap(),
    );
    // Give the handler a chance to start, then abandon it.
    let handle = tokio::spawn(fut);
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
}
