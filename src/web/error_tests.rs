use super::*;

/// Convert a `WebError` into `(status, json body)`.
async fn parts(err: WebError) -> (StatusCode, serde_json::Value) {
    let response = err.into_response();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let json = serde_json::from_slice(&body).expect("json body");
    (status, json)
}

#[tokio::test]
async fn into_response_unauthorized() {
    let (status, json) = parts(WebError::Unauthorized).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"], "Unauthorized");
}

#[tokio::test]
async fn into_response_not_found() {
    let (status, json) = parts(WebError::NotFound("panel")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "panel");
}

#[tokio::test]
async fn into_response_bad_request() {
    let (status, json) = parts(WebError::BadRequest("bad base64".into())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"], "bad base64");
}

#[tokio::test]
async fn into_response_server_unavailable() {
    let (status, json) = parts(WebError::ServerUnavailable).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"], "Server unavailable");
}

#[tokio::test]
async fn into_response_upstream_preserves_status() {
    let (status, json) =
        parts(WebError::Upstream(StatusCode::SERVICE_UNAVAILABLE, "down".into())).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json["error"], "down");
}

#[tokio::test]
async fn into_response_internal() {
    let (status, json) = parts(WebError::Internal("boom".into())).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"], "boom");
}

#[test]
fn display_all_variants() {
    assert_eq!(WebError::Unauthorized.to_string(), "Unauthorized");
    assert_eq!(WebError::NotFound("s").to_string(), "Not found: s");
    assert_eq!(WebError::BadRequest("b".into()).to_string(), "Bad request: b");
    assert_eq!(WebError::ServerUnavailable.to_string(), "Server unavailable");
    assert_eq!(
        WebError::Upstream(StatusCode::BAD_GATEWAY, "u".into()).to_string(),
        "Upstream 502 Bad Gateway: u"
    );
    assert_eq!(WebError::Internal("i".into()).to_string(), "Internal error: i");
}

#[test]
fn debug_format_works() {
    let s = format!("{:?}", WebError::NotFound("session"));
    assert!(s.contains("NotFound"));
    let s2 = format!("{:?}", WebError::Upstream(StatusCode::OK, "x".into()));
    assert!(s2.contains("Upstream"));
}

#[test]
fn implements_std_error_trait() {
    let err: Box<dyn std::error::Error> = Box::new(WebError::Internal("z".into()));
    assert!(err.source().is_none());
    assert!(err.to_string().contains("Internal error"));
}

#[test]
fn web_result_alias_roundtrip() {
    let ok: WebResult<u32> = Ok(7);
    let err: WebResult<u32> = Err(WebError::BadRequest("nope".into()));
    assert_eq!(ok.unwrap(), 7);
    assert!(err.is_err());
}
