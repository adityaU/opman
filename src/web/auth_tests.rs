//! Generated coverage tests for `auth.rs` (JWT + AuthUser extractor + manual check).
//!
//! Added as a separate `#[path]` module so it never collides with the existing
//! `mod tests` in `auth.rs`.

use super::*;

use axum::extract::FromRequestParts;
use axum::http::{header, HeaderValue};

fn secret() -> Vec<u8> {
    b"another-test-secret-key-32-bytes".to_vec()
}

/// Craft a JWT with a caller-chosen `sub`/`exp`, correctly signed with `secret`.
fn craft_jwt(secret: &[u8], sub: &str, exp: u64) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let header_b64 = B64.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = format!(r#"{{"sub":"{sub}","exp":{exp}}}"#);
    let payload_b64 = B64.encode(payload.as_bytes());
    let unsigned = format!("{header_b64}.{payload_b64}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
    mac.update(unsigned.as_bytes());
    let sig = B64.encode(mac.finalize().into_bytes());
    format!("{unsigned}.{sig}")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// ── JWT round-trip / expiry / tamper ───────────────────────────────

#[test]
fn create_verify_round_trip_gen() {
    let s = secret();
    let token = create_jwt("carol", &s).unwrap();
    assert_eq!(verify_jwt(&token, &s), Some("carol".to_string()));
    assert_eq!(token.split('.').count(), 3);
}

#[test]
fn expiry_constant_is_one_day() {
    assert_eq!(JWT_EXPIRY_SECS, 86400);
}

#[test]
fn expired_token_rejected() {
    let s = secret();
    // exp well in the past
    let token = craft_jwt(&s, "dave", 1);
    assert_eq!(verify_jwt(&token, &s), None);
}

#[test]
fn future_token_accepted() {
    let s = secret();
    let token = craft_jwt(&s, "erin", now_secs() + 500);
    assert_eq!(verify_jwt(&token, &s), Some("erin".to_string()));
}

#[test]
fn signature_length_mismatch_rejected() {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    let s = secret();
    // Valid header + payload but a short (3-byte) signature → length mismatch branch.
    let header_b64 = B64.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload_b64 = B64.encode(format!(r#"{{"sub":"x","exp":{}}}"#, now_secs() + 100).as_bytes());
    let short_sig = B64.encode([1u8, 2, 3]);
    let token = format!("{header_b64}.{payload_b64}.{short_sig}");
    assert_eq!(verify_jwt(&token, &s), None);
}

#[test]
fn valid_signature_but_non_claims_payload_rejected() {
    // Payload signs correctly but does not deserialize into JwtClaims.
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let s = secret();
    let header_b64 = B64.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload_b64 = B64.encode(b"{}"); // missing sub/exp
    let unsigned = format!("{header_b64}.{payload_b64}");
    let mut mac = Hmac::<Sha256>::new_from_slice(&s).unwrap();
    mac.update(unsigned.as_bytes());
    let sig = B64.encode(mac.finalize().into_bytes());
    let token = format!("{unsigned}.{sig}");
    assert_eq!(verify_jwt(&token, &s), None);
}

#[test]
fn wrong_secret_rejected_gen() {
    let token = create_jwt("frank", &secret()).unwrap();
    assert_eq!(
        verify_jwt(&token, b"totally-different-secret-key-xyz"),
        None
    );
}

#[test]
fn malformed_tokens_rejected_gen() {
    let s = secret();
    assert_eq!(verify_jwt("", &s), None);
    assert_eq!(verify_jwt("only.two", &s), None);
    assert_eq!(verify_jwt("a.b.c.d.e", &s), None);
    // parts[2] not valid base64 for the sig
    let token = create_jwt("x", &s).unwrap();
    let mut parts: Vec<&str> = token.split('.').collect();
    parts[2] = "@@@not_base64@@@";
    assert_eq!(verify_jwt(&parts.join("."), &s), None);
    // parts[1] not valid base64 but sig valid-length? decode of payload fails.
    let bad_payload = format!("{}.@@@.{}", "aGVhZGVy", "c2ln");
    assert_eq!(verify_jwt(&bad_payload, &s), None);
}

// ── extract_cookie_token ───────────────────────────────────────────

#[test]
fn extract_cookie_token_variants() {
    let mut h = axum::http::HeaderMap::new();
    // absent cookie header
    assert_eq!(extract_cookie_token(&h), None);
    // present among several, with surrounding whitespace (pair is trimmed)
    h.insert(
        header::COOKIE,
        HeaderValue::from_static("foo=bar; opman_token=abc123 ; other=9"),
    );
    assert_eq!(extract_cookie_token(&h), Some("abc123".to_string()));
    // cookie header present but no opman_token
    let mut h2 = axum::http::HeaderMap::new();
    h2.insert(header::COOKIE, HeaderValue::from_static("a=1; b=2"));
    assert_eq!(extract_cookie_token(&h2), None);
}

#[test]
fn extract_cookie_token_exact_value() {
    let mut h = axum::http::HeaderMap::new();
    h.insert(header::COOKIE, HeaderValue::from_static("opman_token=xyz"));
    assert_eq!(extract_cookie_token(&h), Some("xyz".to_string()));
}

// ── AuthUser extractor ─────────────────────────────────────────────

fn parts_with(headers: Vec<(header::HeaderName, &str)>, uri: &str) -> axum::http::request::Parts {
    let mut b = axum::http::Request::builder().uri(uri);
    for (k, v) in headers {
        b = b.header(k, v);
    }
    let req = b.body(axum::body::Body::empty()).unwrap();
    req.into_parts().0
}

#[tokio::test]
async fn extractor_no_auth_configured_allows_anonymous() {
    let state = crate::web::test_support::test_server_state();
    let mut parts = parts_with(vec![], "/api/x");
    let user = AuthUser::from_request_parts(&mut parts, &state)
        .await
        .unwrap();
    assert_eq!(user.subject, "anonymous");
}

#[tokio::test]
async fn extractor_missing_token_unauthorized() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let mut parts = parts_with(vec![], "/api/x");
    let res = AuthUser::from_request_parts(&mut parts, &state).await;
    assert!(matches!(res, Err(WebError::Unauthorized)));
}

#[tokio::test]
async fn extractor_bearer_valid() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let token = create_jwt("u", &state.jwt_secret).unwrap();
    let mut parts = parts_with(
        vec![(header::AUTHORIZATION, &format!("Bearer {token}"))],
        "/api/x",
    );
    let user = AuthUser::from_request_parts(&mut parts, &state)
        .await
        .unwrap();
    assert_eq!(user.subject, "u");
}

#[tokio::test]
async fn extractor_bearer_invalid_unauthorized() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let mut parts = parts_with(
        vec![(header::AUTHORIZATION, "Bearer not-a-real-token")],
        "/api/x",
    );
    assert!(matches!(
        AuthUser::from_request_parts(&mut parts, &state).await,
        Err(WebError::Unauthorized)
    ));
}

#[tokio::test]
async fn extractor_non_bearer_auth_falls_through_to_unauthorized() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    // "Basic ..." has no "Bearer " prefix → strip returns None → no cookie/query → 401.
    let mut parts = parts_with(
        vec![(header::AUTHORIZATION, "Basic Zm9vOmJhcg==")],
        "/api/x",
    );
    assert!(matches!(
        AuthUser::from_request_parts(&mut parts, &state).await,
        Err(WebError::Unauthorized)
    ));
}

#[tokio::test]
async fn extractor_cookie_token_valid() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let token = create_jwt("cookieuser", &state.jwt_secret).unwrap();
    let mut parts = parts_with(
        vec![(header::COOKIE, &format!("opman_token={token}"))],
        "/api/x",
    );
    let user = AuthUser::from_request_parts(&mut parts, &state)
        .await
        .unwrap();
    assert_eq!(user.subject, "cookieuser");
}

#[tokio::test]
async fn extractor_query_token_valid() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let token = create_jwt("queryuser", &state.jwt_secret).unwrap();
    // STANDARD-base64 JWTs may contain `+` / `/` / `=`; percent-encode so the
    // extractor's form_urlencoded parse round-trips the exact token.
    let enc: String = url::form_urlencoded::byte_serialize(token.as_bytes()).collect();
    let uri = format!("/api/events?token={enc}");
    let mut parts = parts_with(vec![], &uri);
    let user = AuthUser::from_request_parts(&mut parts, &state)
        .await
        .unwrap();
    assert_eq!(user.subject, "queryuser");
}

// ── check_auth_manual ──────────────────────────────────────────────

#[tokio::test]
async fn check_auth_manual_no_auth_configured() {
    let state = crate::web::test_support::test_server_state();
    let h = axum::http::HeaderMap::new();
    assert!(check_auth_manual(&state, &h, &None));
}

#[tokio::test]
async fn check_auth_manual_bearer_valid() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let token = create_jwt("u", &state.jwt_secret).unwrap();
    let mut h = axum::http::HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    assert!(check_auth_manual(&state, &h, &None));
}

#[tokio::test]
async fn check_auth_manual_cookie_valid() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let token = create_jwt("u", &state.jwt_secret).unwrap();
    let mut h = axum::http::HeaderMap::new();
    h.insert(
        header::COOKIE,
        HeaderValue::from_str(&format!("opman_token={token}")).unwrap(),
    );
    assert!(check_auth_manual(&state, &h, &None));
}

#[tokio::test]
async fn check_auth_manual_query_token_valid() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let token = create_jwt("u", &state.jwt_secret).unwrap();
    let h = axum::http::HeaderMap::new();
    assert!(check_auth_manual(&state, &h, &Some(token)));
}

#[tokio::test]
async fn check_auth_manual_no_token_false() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let h = axum::http::HeaderMap::new();
    assert!(!check_auth_manual(&state, &h, &None));
}

#[tokio::test]
async fn check_auth_manual_invalid_token_false() {
    let state = crate::web::test_support::test_server_state_with_auth("u", "p");
    let h = axum::http::HeaderMap::new();
    assert!(!check_auth_manual(&state, &h, &Some("garbage".to_string())));
}
