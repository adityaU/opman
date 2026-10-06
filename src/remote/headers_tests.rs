use super::*;
use axum::http::HeaderValue;

fn map(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        headers.append(*name, HeaderValue::from_static(value));
    }
    headers
}

#[test]
fn request_drops_hop_by_hop_and_home_credentials() {
    let source = map(&[
        ("host", "home:1"),
        ("authorization", "Bearer home-token"),
        ("cookie", "opman_token=home"),
        ("connection", "keep-alive, x-private"),
        ("keep-alive", "timeout=5"),
        ("transfer-encoding", "chunked"),
        ("upgrade", "h2c"),
        ("te", "trailers"),
        ("x-private", "1"),
        ("content-type", "application/json"),
        ("accept", "text/event-stream"),
        ("x-multi", "a"),
        ("x-multi", "b"),
    ]);
    let out = upstream_request(&source);
    let names: Vec<&str> = out.keys().map(|k| k.as_str()).collect();
    assert_eq!(names, ["content-type", "accept", "x-multi"]);
    assert_eq!(out.get_all("x-multi").iter().count(), 2);
}

#[test]
fn response_drops_set_cookie_keeps_the_rest() {
    let source = map(&[
        ("set-cookie", "opman_token=remote"),
        ("transfer-encoding", "chunked"),
        ("content-type", "text/event-stream"),
        ("cache-control", "no-cache"),
        ("content-encoding", "gzip"),
    ]);
    let out = downstream_response(&source);
    assert!(out.get("set-cookie").is_none());
    assert!(out.get("transfer-encoding").is_none());
    assert_eq!(out["content-type"], "text/event-stream");
    assert_eq!(out["cache-control"], "no-cache");
    assert_eq!(out["content-encoding"], "gzip");
}

#[test]
fn query_strips_home_token() {
    assert_eq!(upstream_query(None), None);
    assert_eq!(upstream_query(Some("")), None);
    assert_eq!(upstream_query(Some("token=abc")), None);
    assert_eq!(
        upstream_query(Some("a=1&token=abc&b=2")).as_deref(),
        Some("a=1&b=2")
    );
    assert_eq!(
        upstream_query(Some("tokenish=1&&x")).as_deref(),
        Some("tokenish=1&x")
    );
}
