use super::*;

use crate::web::test_support::test_server_state;
use crate::web::types::{WebThemeColors, WebThemePair};

// ── Pure helper tests ───────────────────────────────────────────────

#[test]
fn etag_from_hash_zero() {
    let etag = etag_from_hash(&[0u8; 32]);
    // 16 bytes -> 32 hex chars, wrapped in quotes.
    assert_eq!(etag, "\"00000000000000000000000000000000\"");
}

#[test]
fn etag_from_hash_distinct_bytes() {
    let mut hash = [0u8; 32];
    hash[0] = 0xab;
    hash[15] = 0x0f;
    let etag = etag_from_hash(&hash);
    assert!(etag.starts_with("\"ab"));
    assert!(etag.ends_with("0f\""));
    // Only the first 16 bytes are used -> byte 16 onward ignored.
    let mut hash2 = hash;
    hash2[16] = 0xff;
    assert_eq!(etag_from_hash(&hash2), etag);
}

#[test]
fn is_not_modified_matches() {
    let mut headers = HeaderMap::new();
    headers.insert(header::IF_NONE_MATCH, "\"abc123\"".parse().unwrap());
    assert!(is_not_modified(&headers, "\"abc123\""));
}

#[test]
fn is_not_modified_no_header() {
    let headers = HeaderMap::new();
    assert!(!is_not_modified(&headers, "\"abc123\""));
}

#[test]
fn is_not_modified_mismatch() {
    let mut headers = HeaderMap::new();
    headers.insert(header::IF_NONE_MATCH, "\"other\"".parse().unwrap());
    assert!(!is_not_modified(&headers, "\"abc123\""));
}

#[test]
fn build_ok_returns_body() {
    let resp = build_ok(Response::builder().status(StatusCode::OK), Body::from("hi"));
    assert_eq!(resp.status(), StatusCode::OK);
}

#[test]
fn react_assets_get_known_and_missing() {
    // index.html is always embedded from web-ui/dist.
    assert!(ReactAssets::get("index.html").is_some());
    assert!(ReactAssets::get("definitely-not-a-real-asset.xyz").is_none());
}

// ── serve_react integration tests ───────────────────────────────────

fn colors(bg: &str, primary: &str) -> WebThemeColors {
    WebThemeColors {
        primary: primary.into(),
        secondary: "#111111".into(),
        accent: "#222222".into(),
        background: bg.into(),
        background_panel: "#333333".into(),
        background_element: "#444444".into(),
        text: "#555555".into(),
        text_muted: "#666666".into(),
        border: "#777777".into(),
        border_active: "#888888".into(),
        border_subtle: "#999999".into(),
        error: "#aa0000".into(),
        warning: "#bb0000".into(),
        success: "#00aa00".into(),
        info: "#0000aa".into(),
    }
}

fn theme_pair(bg: &str, primary: &str) -> WebThemePair {
    WebThemePair {
        dark: colors(bg, primary),
        light: colors(bg, primary),
    }
}

async fn call(state: &ServerState, path: &str, if_none_match: Option<&str>) -> Response<Body> {
    let mut headers = HeaderMap::new();
    if let Some(v) = if_none_match {
        headers.insert(header::IF_NONE_MATCH, v.parse().unwrap());
    }
    let uri: axum::http::Uri = path.parse().unwrap();
    serve_react(State(state.clone()), headers, uri)
        .await
        .into_response()
}

async fn body_string(resp: Response<Body>) -> (StatusCode, HeaderMap, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

#[tokio::test]
async fn serves_index_fallback_for_root() {
    let state = test_server_state();
    let (status, headers, body) = body_string(call(&state, "/", None).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "text/html");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn serves_index_fallback_for_unknown_spa_route() {
    let state = test_server_state();
    // No asset named this -> SPA fallback to index.html.
    let (status, headers, _) = body_string(call(&state, "/some/deep/spa/route", None).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "text/html");
}

#[tokio::test]
async fn serves_static_asset_with_etag_and_304() {
    let state = test_server_state();
    // index.html requested by exact name hits the static-asset (ETag) branch.
    let resp = call(&state, "/index.html", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let etag = resp
        .headers()
        .get(header::ETAG)
        .expect("etag header")
        .to_str()
        .unwrap()
        .to_string();
    assert!(etag.starts_with('"'));

    // Re-request with matching If-None-Match -> 304 Not Modified.
    let resp2 = call(&state, "/index.html", Some(&etag)).await;
    assert_eq!(resp2.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(resp2.headers().get(header::ETAG).unwrap(), etag.as_str());
}

#[tokio::test]
async fn serves_manifest_json() {
    let state = test_server_state();
    let (status, headers, _) = body_string(call(&state, "/manifest.json", None).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::CONTENT_TYPE).unwrap(),
        "application/manifest+json"
    );
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-cache");
}

#[tokio::test]
async fn serves_service_worker() {
    let state = test_server_state();
    let (status, headers, _) = body_string(call(&state, "/sw.js", None).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::CONTENT_TYPE).unwrap(),
        "application/javascript"
    );
    assert!(headers.get("Service-Worker-Allowed").is_some());
}

#[tokio::test]
async fn favicon_without_theme_falls_to_static_branch() {
    // No theme set -> theme.primary/bg are None -> dynamic favicon branch is
    // skipped and favicon.svg is served as a plain static asset with an ETag.
    let state = test_server_state();
    let resp = call(&state, "/favicon.svg", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/svg+xml"
    );
    assert!(resp.headers().get(header::ETAG).is_some());
}

#[tokio::test]
async fn favicon_with_theme_uses_dynamic_branch() {
    let state = test_server_state();
    state
        .web_state
        .set_theme(theme_pair("#010203", "#ff8800"))
        .await;
    let resp = call(&state, "/favicon.svg", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/svg+xml"
    );
    // Dynamic branch does not attach an ETag.
    assert!(resp.headers().get(header::ETAG).is_none());
    assert_eq!(
        resp.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
}

#[tokio::test]
async fn index_and_manifest_with_instance_name_and_theme() {
    let mut state = test_server_state();
    state.instance_name = Some("My Instance".to_string());
    state
        .web_state
        .set_theme(theme_pair("#0B0E14", "#ff8800"))
        .await;

    // index fallback with name + theme replacements.
    let (status, _, _) = body_string(call(&state, "/", None).await).await;
    assert_eq!(status, StatusCode::OK);

    // manifest with name + theme replacements.
    let (mstatus, _, _) = body_string(call(&state, "/manifest.json", None).await).await;
    assert_eq!(mstatus, StatusCode::OK);
}

#[tokio::test]
async fn resolve_theme_none_and_some() {
    let state = test_server_state();
    let t = resolve_theme(&state).await;
    assert!(t.bg.is_none());
    assert!(t.primary.is_none());

    state
        .web_state
        .set_theme(theme_pair("#123456", "#abcdef"))
        .await;
    let t2 = resolve_theme(&state).await;
    assert_eq!(t2.bg.as_deref(), Some("#123456"));
    assert_eq!(t2.primary.as_deref(), Some("#abcdef"));
}
