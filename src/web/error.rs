//! Unified error type for the web module.
//!
//! All handler functions return `Result<impl IntoResponse, WebError>` so that
//! Axum can automatically convert errors into appropriate HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

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
        let (status, body) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.to_string()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::ServerUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server unavailable".to_string(),
            ),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        (status, body).into_response()
    }
}

/// Convenience alias for handler return types.
pub type WebResult<T> = Result<T, WebError>;
