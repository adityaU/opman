//! Unified error type for the web module.
//!
//! All handler functions return `Result<impl IntoResponse, WebError>` so that
//! Axum can automatically convert errors into appropriate HTTP responses.
//! Error responses are always JSON: `{ "error": "<message>" }`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

/// JSON body for error responses.
#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

/// Unified error type for web handlers.
#[derive(Debug)]
pub enum WebError {
    /// Client sent invalid credentials or missing/expired JWT.
    Unauthorized,
    /// Request references a resource that doesn't exist (session, panel, etc.).
    NotFound(&'static str),
    /// Request body failed validation (bad base64, unknown panel name, etc.).
    BadRequest(String),
    /// The main TUI loop is unreachable (channel closed or oneshot dropped).
    #[allow(dead_code)]
    ServerUnavailable,
    /// Catch-all for unexpected internal failures.
    Internal(String),
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Self::ServerUnavailable => write!(f, "Server unavailable"),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for WebError {}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.to_string()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::ServerUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server unavailable".to_string(),
            ),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

/// Convenience alias for handler return types.
pub type WebResult<T> = Result<T, WebError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: convert a WebError into an HTTP response and extract status + JSON body.
    ///
    /// Uses `axum::body::to_bytes` (re-exported from http-body-util) to consume the body.
    async fn error_to_parts(err: WebError) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body to_bytes");
        let json: serde_json::Value =
            serde_json::from_slice(&body_bytes).expect("body should be JSON");
        (status, json)
    }

    #[tokio::test]
    async fn unauthorized_returns_401() {
        let (status, json) = error_to_parts(WebError::Unauthorized).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "Unauthorized");
    }

    #[tokio::test]
    async fn not_found_returns_404() {
        let (status, json) = error_to_parts(WebError::NotFound("session")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "session");
    }

    #[tokio::test]
    async fn bad_request_returns_400() {
        let (status, json) =
            error_to_parts(WebError::BadRequest("invalid base64".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "invalid base64");
    }

    #[tokio::test]
    async fn server_unavailable_returns_500() {
        let (status, json) = error_to_parts(WebError::ServerUnavailable).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["error"], "Server unavailable");
    }

    #[tokio::test]
    async fn internal_returns_500_with_message() {
        let (status, json) =
            error_to_parts(WebError::Internal("db crashed".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["error"], "db crashed");
    }

    #[test]
    fn display_variants() {
        assert_eq!(WebError::Unauthorized.to_string(), "Unauthorized");
        assert_eq!(
            WebError::NotFound("session").to_string(),
            "Not found: session"
        );
        assert_eq!(
            WebError::BadRequest("oops".into()).to_string(),
            "Bad request: oops"
        );
        assert_eq!(
            WebError::ServerUnavailable.to_string(),
            "Server unavailable"
        );
        assert_eq!(
            WebError::Internal("boom".into()).to_string(),
            "Internal error: boom"
        );
    }

    #[test]
    fn web_error_is_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(WebError::BadRequest("test".into()));
        assert!(err.to_string().contains("Bad request"));
    }

    #[tokio::test]
    async fn error_body_is_always_json_object_with_error_key() {
        // All variants should produce {"error": "..."} — verify shape
        let variants: Vec<WebError> = vec![
            WebError::Unauthorized,
            WebError::NotFound("x"),
            WebError::BadRequest("y".into()),
            WebError::ServerUnavailable,
            WebError::Internal("z".into()),
        ];
        for variant in variants {
            let (_, json) = error_to_parts(variant).await;
            assert!(
                json.get("error").is_some(),
                "Missing 'error' key in JSON body"
            );
            assert!(
                json["error"].is_string(),
                "'error' should be a string"
            );
        }
    }
}
