//! Unified error type for the web module.
//!
//! All handler functions return `Result<impl IntoResponse, WebError>` so that
//! Axum can automatically convert errors into appropriate HTTP responses.
//! Error responses are always JSON: `{ "error": "<message>" }`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

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
    /// The main TUI loop is unreachable (channel closed or oneshot dropped).
    #[allow(dead_code)]
    ServerUnavailable,
    /// Upstream (opencode server) returned an error — preserve its status code.
    Upstream(StatusCode, String),
    /// Chromium is not installed, or an explicit browser path is unusable.
    BrowserUnavailable(BrowserInstallGuide),
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
            Self::Upstream(status, msg) => write!(f, "Upstream {}: {}", status, msg),
            Self::BrowserUnavailable(_) => write!(f, "Browser engine unavailable"),
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
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorBody {
                    error: "Server unavailable".to_string(),
                    code: None,
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
        let (status, json) = error_to_parts(WebError::BadRequest("invalid base64".into())).await;
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
    async fn browser_unavailable_returns_platform_setup() {
        let (status, json) = error_to_parts(WebError::BrowserUnavailable(
            crate::browser::BrowserUnavailable::NoBrowser.install_guide(),
        ))
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json["code"], "browser_unavailable");
        assert!(json["browser_setup"]["title"].as_str().is_some());
        assert_eq!(json["browser_setup"]["issue"]["kind"], "no_browser");
    }

    #[tokio::test]
    async fn browser_override_failure_returns_the_offending_path() {
        let unavailable = crate::browser::BrowserUnavailable::OverrideMissing(
            std::path::PathBuf::from("/wrong/chromium"),
        );
        let (status, json) =
            error_to_parts(WebError::BrowserUnavailable(unavailable.install_guide())).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json["browser_setup"]["issue"]["kind"], "override_missing");
        assert_eq!(json["browser_setup"]["issue"]["path"], "/wrong/chromium");
    }

    #[tokio::test]
    async fn internal_returns_500_with_message() {
        let (status, json) = error_to_parts(WebError::Internal("db crashed".into())).await;
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
            WebError::Upstream(StatusCode::BAD_GATEWAY, "oops".into()).to_string(),
            "Upstream 502 Bad Gateway: oops"
        );
        assert_eq!(
            WebError::Internal("boom".into()).to_string(),
            "Internal error: boom"
        );
    }

    #[tokio::test]
    async fn upstream_preserves_status_code() {
        let (status, json) = error_to_parts(WebError::Upstream(
            StatusCode::BAD_GATEWAY,
            "upstream died".into(),
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(json["error"], "upstream died");
    }

    #[tokio::test]
    async fn upstream_not_found_returns_404() {
        let (status, json) = error_to_parts(WebError::Upstream(
            StatusCode::NOT_FOUND,
            "session not found".into(),
        ))
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"], "session not found");
    }

    #[test]
    fn web_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(WebError::BadRequest("test".into()));
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
            WebError::Upstream(StatusCode::BAD_GATEWAY, "upstream died".into()),
            WebError::Internal("z".into()),
        ];
        for variant in variants {
            let (_, json) = error_to_parts(variant).await;
            assert!(
                json.get("error").is_some(),
                "Missing 'error' key in JSON body"
            );
            assert!(json["error"].is_string(), "'error' should be a string");
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
