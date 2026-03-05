//! JWT authentication: token creation, verification, and Axum extractor.
//!
//! Instead of a `require_auth!` macro in every handler, authentication is
//! enforced via an Axum `FromRequestParts` extractor (`AuthUser`). Handlers
//! that need auth simply include `_auth: AuthUser` in their signature.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::header;
use axum::http::request::Parts;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use super::error::WebError;
use super::types::ServerState;

type HmacSha256 = Hmac<Sha256>;

/// JWT tokens are valid for 24 hours.
const JWT_EXPIRY_SECS: u64 = 86400;

// ── JWT claims ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    exp: u64,
}

// ── Public API ──────────────────────────────────────────────────────

/// Create a signed JWT for the given username.
///
/// Returns an error if the HMAC key is somehow invalid (should never
/// happen with a 32-byte random secret, but we don't panic).
pub fn create_jwt(username: &str, secret: &[u8]) -> Result<String, WebError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| WebError::Internal(format!("Clock error: {e}")))?
        .as_secs();

    let claims = JwtClaims {
        sub: username.to_string(),
        exp: now + JWT_EXPIRY_SECS,
    };

    let header_b64 = BASE64.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload_b64 = BASE64.encode(
        serde_json::to_vec(&claims)
            .map_err(|e| WebError::Internal(format!("JWT serialize: {e}")))?,
    );
    let unsigned = format!("{header_b64}.{payload_b64}");

    let mut mac =
        HmacSha256::new_from_slice(secret).map_err(|e| WebError::Internal(format!("HMAC: {e}")))?;
    mac.update(unsigned.as_bytes());
    let sig = BASE64.encode(mac.finalize().into_bytes());

    Ok(format!("{unsigned}.{sig}"))
}

/// Verify a JWT and return the subject (username) if valid.
pub fn verify_jwt(token: &str, secret: &[u8]) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let unsigned = format!("{}.{}", parts[0], parts[1]);
    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(unsigned.as_bytes());
    let expected_sig = BASE64.encode(mac.finalize().into_bytes());
    if expected_sig != parts[2] {
        return None;
    }

    let payload_bytes = BASE64.decode(parts[1]).ok()?;
    let claims: JwtClaims = serde_json::from_slice(&payload_bytes).ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    if claims.exp < now {
        return None;
    }

    Some(claims.sub)
}

// ── Axum extractor ──────────────────────────────────────────────────

/// An authenticated user, extracted from the request.
///
/// Include this in a handler's arguments to enforce JWT authentication:
///
/// ```ignore
/// async fn my_handler(_auth: AuthUser, ...) -> impl IntoResponse { ... }
/// ```
///
/// If auth is not configured (empty username), all requests are allowed
/// and the subject is "anonymous".
pub struct AuthUser {
    #[allow(dead_code)]
    pub subject: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    ServerState: axum::extract::FromRef<S>,
{
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let server_state = ServerState::from_ref(state);

        // No auth configured → allow everyone
        if server_state.username.is_empty() {
            return Ok(AuthUser {
                subject: "anonymous".to_string(),
            });
        }

        // Try Authorization: Bearer <token> header
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        // Fall back to ?token= query parameter (for SSE connections)
        let token = token.or_else(|| {
            parts
                .uri
                .query()
                .and_then(|q| {
                    url::form_urlencoded::parse(q.as_bytes())
                        .find(|(k, _)| k == "token")
                        .map(|(_, v)| v.to_string())
                })
        });

        let token = token.ok_or(WebError::Unauthorized)?;
        let subject =
            verify_jwt(&token, &server_state.jwt_secret).ok_or(WebError::Unauthorized)?;

        Ok(AuthUser { subject })
    }
}

/// Check auth from headers + optional query token (for non-extractor use in SSE).
pub fn check_auth_manual(
    state: &ServerState,
    headers: &axum::http::HeaderMap,
    query_token: &Option<String>,
) -> bool {
    if state.username.is_empty() {
        return true;
    }

    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| query_token.clone());

    token
        .and_then(|t| verify_jwt(&t, &state.jwt_secret))
        .is_some()
}
