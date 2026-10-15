//! Unified error type for the web module.
//!
//! All handler functions return `Result<impl IntoResponse, WebError>` so that
//! Axum can automatically convert errors into appropriate HTTP responses.
//! Error responses are always JSON: `{ "error": "<message>" }`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

use crate::app::try_base_url;
use crate::browser::BrowserInstallGuide;

/// JSON body for error responses.
#[derive(Serialize)]
struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    browser_setup: Option<BrowserInstallGuide>,
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
    /// The default runner is not available yet.
    ServerUnavailable,
    /// Upstream (opencode server) returned an error — preserve its status code.
    Upstream(StatusCode, String),
    /// Chromium is not installed, or an explicit browser path is unusable.
    BrowserUnavailable(BrowserInstallGuide),
    /// The request needs a runner that is switched off in settings. 409 with a code the
    /// browser recognises, so it can say so and offer to turn the runner back on.
    RunnerDisabled(crate::runner::RunnerDisabled),
    /// Catch-all for unexpected internal failures.
    Internal(String),
}

impl WebError {
    /// A runner call failed: a switched-off runner becomes [`Self::RunnerDisabled`],
    /// anything else stays an internal error.
    pub fn runner(error: anyhow::Error) -> Self {
        match crate::runner::RunnerDisabled::find(&error) {
            Some(disabled) => Self::RunnerDisabled(disabled.clone()),
            None => Self::Internal(format!("Runner error: {error}")),
        }
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Self::ServerUnavailable => write!(f, "Server unavailable"),
            Self::Upstream(status, msg) => write!(f, "Upstream {}: {}", status, msg),
            Self::BrowserUnavailable(_) => write!(f, "Browser engine unavailable"),
            Self::RunnerDisabled(disabled) => write!(f, "{disabled}"),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for WebError {}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    error: "Unauthorized".to_string(),
                    code: None,
                    browser_setup: None,
                },
            ),
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    error: msg.to_string(),
                    code: None,
                    browser_setup: None,
                },
            ),
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    error: msg,
                    code: None,
                    browser_setup: None,
                },
            ),
            Self::ServerUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    error: "Server unavailable".to_string(),
                    code: Some("server_unavailable"),
                    browser_setup: None,
                },
            ),
            Self::Upstream(status, msg) => (
                status,
                ErrorBody {
                    error: msg,
                    code: None,
                    browser_setup: None,
                },
            ),
            Self::BrowserUnavailable(guide) => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    error: "Browser engine unavailable".to_string(),
                    code: Some("browser_unavailable"),
                    browser_setup: Some(guide),
                },
            ),
            Self::RunnerDisabled(disabled) => (
                StatusCode::CONFLICT,
                ErrorBody {
                    error: disabled.to_string(),
                    code: Some("runner_disabled"),
                    browser_setup: None,
                },
            ),
            Self::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorBody {
                    error: msg,
                    code: None,
                    browser_setup: None,
                },
            ),
        };
        (status, Json(body)).into_response()
    }
}

/// Resolve the default runner for a web request without turning a lazy-start race into a
/// process abort. Handlers that need a runner can use this at the point of the request.
pub(crate) fn base_url_or_unavailable() -> WebResult<&'static str> {
    try_base_url().ok_or(WebError::ServerUnavailable)
}

/// Convenience alias for handler return types.
pub type WebResult<T> = Result<T, WebError>;

#[cfg(test)]
#[path = "error_response_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
