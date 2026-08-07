//! The loopback callback: the only part of the flow a browser touches.
//!
//! Adapted from the Slack flow in `src/integrations/slack/auth/oauth.rs` — bind before
//! opening the browser so there is no race, time out the accept, and parse only the
//! request line rather than pulling in an HTTP server for one request.

use std::collections::BTreeMap;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

use super::OAuthError;

const CALLBACK_PATH: &str = "/callback";
const BROWSER_TIMEOUT: Duration = Duration::from_secs(300);

// ── the browser leg ─────────────────────────────────────────────────────────────────

/// Bind first, so the advertised port always has a listener behind it.
pub(super) async fn bind(port: u16) -> Result<(TcpListener, String), OAuthError> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(OAuthError::Io)?;
    let bound = listener.local_addr().map_err(OAuthError::Io)?.port();
    Ok((listener, format!("http://127.0.0.1:{bound}{CALLBACK_PATH}")))
}

/// Accept one request and return its query parameters.
pub(super) async fn wait_for_callback(listener: TcpListener) -> Result<BTreeMap<String, String>, OAuthError> {
    let (mut stream, _) = tokio::time::timeout(BROWSER_TIMEOUT, listener.accept())
        .await
        .map_err(|_| OAuthError::Discovery("timed out waiting for the browser".into()))?
        .map_err(OAuthError::Io)?;
    let mut buf = vec![0_u8; 4096];
    let read = stream.read(&mut buf).await.map_err(OAuthError::Io)?;
    let request = String::from_utf8_lossy(&buf[..read]);
    let query = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|target| target.split_once('?').map(|(_, q)| q.to_string()));
    let body = "<html><body><h3>opman: you can close this tab.</h3></body></html>";
    let _ = stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await;
    let _ = stream.flush().await;
    Ok(parse_query(query.as_deref().unwrap_or_default()))
}

pub(crate) fn parse_query(query: &str) -> BTreeMap<String, String> {
    Url::parse(&format!("http://x/?{query}"))
        .map(|url| {
            url.query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// RFC 9207, in the order the spec requires.
///
/// `state` first, then `iss`, and only then `error` — because on an issuer mismatch a
/// client MUST NOT act on or display the server-supplied error fields.
pub(crate) fn validate_response(
    params: &BTreeMap<String, String>,
    expected_state: &str,
    expected_issuer: &str,
    iss_required: bool,
) -> Result<String, OAuthError> {
    match params.get("state") {
        Some(state) if state == expected_state => {}
        _ => return Err(OAuthError::StateMismatch),
    }
    match params.get("iss") {
        // Byte-exact: no scheme or host case folding, no trailing-slash normalisation.
        Some(iss) if iss != expected_issuer => return Err(OAuthError::IssuerMismatch),
        None if iss_required => return Err(OAuthError::IssuerMismatch),
        _ => {}
    }
    if let Some(error) = params.get("error") {
        return Err(OAuthError::Denied(error.clone()));
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| OAuthError::Denied("no authorization code returned".into()))
}


#[cfg(test)]
#[path = "callback_tests.rs"]
mod callback_tests;
