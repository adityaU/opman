//! The HTTP half: streamable-HTTP to the remote MCP server, with a fresh credential per
//! request.
//!
//! The access token is deliberately *not* cached here. It is read from the store on every
//! request — a small page-cached file read — so `opman mcp logout` takes effect on the
//! very next tool call rather than whenever this process happens to restart.

use std::sync::Mutex;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use serde_json::Value;

use crate::mcp_oauth::{self, OAuthError, ServerName, TokenStore};
use crate::mcp_registry::spec::Remote;

const SESSION_HEADER: &str = "mcp-session-id";
const PROTOCOL_HEADER: &str = "mcp-protocol-version";
/// One retry only: a refresh either fixes a 401 or the credential is genuinely gone.
const MAX_ATTEMPTS: u8 = 2;

pub(crate) enum UpstreamError {
    /// No usable credential. The caller holds the call open rather than failing outright.
    NeedsLogin,
    /// 403 `insufficient_scope`. Never retried — a refresh cannot widen a grant that
    /// never had the scope.
    NeedsScope(String),
    Transport(String),
}

pub(crate) struct Upstream {
    name: ServerName,
    endpoint: String,
    http: reqwest::Client,
    store: TokenStore,
    extra_headers: HeaderMap,
    session: Mutex<Option<String>>,
    protocol: Mutex<Option<String>>,
}

impl Upstream {
    pub(crate) fn new(name: ServerName, remote: &Remote, store: TokenStore) -> Self {
        let mut extra_headers = HeaderMap::new();
        for (key, value) in remote.literal_headers() {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(&value),
            ) {
                extra_headers.insert(name, value);
            }
        }
        Self {
            name,
            endpoint: remote.url().to_string(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            store,
            extra_headers,
            session: Mutex::new(None),
            protocol: Mutex::new(None),
        }
    }

    pub(crate) fn name(&self) -> &ServerName {
        &self.name
    }

    /// Whether opman currently holds a usable credential — the wait loop's exit condition.
    pub(crate) fn authenticated(&self) -> bool {
        mcp_oauth::is_authenticated(&self.store, &self.name)
    }

    /// Forward one request and return every message to relay.
    pub(crate) async fn send(&self, message: &Value) -> Result<Vec<Value>, UpstreamError> {
        for attempt in 0..MAX_ATTEMPTS {
            let token = match self.token(attempt).await {
                Ok(token) => token,
                Err(OAuthError::LoginRequired) => return Err(UpstreamError::NeedsLogin),
                Err(error) => return Err(UpstreamError::Transport(error.to_string())),
            };
            let response = self
                .post(message, &token)
                .await
                .map_err(|e| UpstreamError::Transport(e.to_string()))?;
            let status = response.status();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                // Attempt 0 refreshes and retries; attempt 1 means the credential really
                // is gone.
                continue;
            }
            if status == reqwest::StatusCode::FORBIDDEN {
                let scope = scope_from_challenge(&response).unwrap_or_default();
                return Err(UpstreamError::NeedsScope(scope));
            }
            self.remember_session(&response);
            if status == reqwest::StatusCode::ACCEPTED {
                return Ok(Vec::new());
            }
            let is_sse = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with("text/event-stream"));
            let body = response
                .text()
                .await
                .map_err(|e| UpstreamError::Transport(e.to_string()))?;
            let values = if is_sse {
                decode_sse(&body)
            } else {
                serde_json::from_str::<Value>(&body)
                    .map(|v| vec![v])
                    .unwrap_or_default()
            };
            self.remember_protocol(&values);
            return Ok(values);
        }
        Err(UpstreamError::NeedsLogin)
    }

    /// Fire a notification and ignore the outcome — there is no reply to relay.
    pub(crate) async fn notify(&self, message: &Value) {
        if let Ok(token) = self.token(0).await {
            let _ = self.post(message, &token).await;
        }
    }

    /// Release the remote session so it is not left dangling for its idle timeout.
    pub(crate) async fn terminate(&self) {
        let Some(session) = self.session() else {
            return;
        };
        if let Ok(token) = self.token(0).await {
            let _ = self
                .http
                .delete(&self.endpoint)
                .bearer_auth(token.expose())
                .header(SESSION_HEADER, session)
                .send()
                .await;
        }
    }

    async fn token(&self, attempt: u8) -> Result<mcp_oauth::Secret, OAuthError> {
        if attempt == 0 {
            return mcp_oauth::access_token(&self.http, &self.store, &self.name).await;
        }
        // A 401 on a token we believed was good: a plain expiry check cannot catch a
        // server-side revocation, so force the exchange.
        mcp_oauth::force_refresh(&self.http, &self.store, &self.name).await
    }

    async fn post(
        &self,
        message: &Value,
        token: &mcp_oauth::Secret,
    ) -> reqwest::Result<reqwest::Response> {
        let mut request = self
            .http
            .post(&self.endpoint)
            .headers(self.extra_headers.clone())
            .bearer_auth(token.expose())
            .header(ACCEPT, "application/json, text/event-stream")
            .json(message);
        if let Some(session) = self.session() {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(protocol) = self.protocol() {
            request = request.header(PROTOCOL_HEADER, protocol);
        }
        request.send().await
    }

    fn session(&self) -> Option<String> {
        self.session.lock().ok().and_then(|g| g.clone())
    }

    fn protocol(&self) -> Option<String> {
        self.protocol.lock().ok().and_then(|g| g.clone())
    }

    fn remember_session(&self, response: &reqwest::Response) {
        let Some(value) = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|v| v.to_str().ok())
        else {
            return;
        };
        if let Ok(mut guard) = self.session.lock() {
            *guard = Some(value.to_string());
        }
    }

    /// Record whatever version the *runner and remote* negotiated, rather than asserting
    /// one of our own — the proxy is a pipe, not a participant.
    fn remember_protocol(&self, values: &[Value]) {
        let Some(version) = values
            .iter()
            .filter_map(|v| v.pointer("/result/protocolVersion"))
            .filter_map(Value::as_str)
            .next()
        else {
            return;
        };
        if let Ok(mut guard) = self.protocol.lock() {
            *guard = Some(version.to_string());
        }
    }
}

fn scope_from_challenge(response: &reqwest::Response) -> Option<String> {
    let header = response
        .headers()
        .get(reqwest::header::WWW_AUTHENTICATE)?
        .to_str()
        .ok()?;
    parse_scope(header)
}

/// Pull `scope="…"` out of a `WWW-Authenticate: Bearer …` challenge.
pub(crate) fn parse_scope(header: &str) -> Option<String> {
    let start = header.find("scope=")? + "scope=".len();
    let rest = &header[start..];
    let rest = rest.strip_prefix('"').unwrap_or(rest);
    let end = rest.find(['"', ',']).unwrap_or(rest.len());
    Some(rest[..end].to_string()).filter(|s| !s.is_empty())
}

/// Decode `data:` payloads out of an SSE body. Handles multi-line folding and CRLF.
pub(crate) fn decode_sse(body: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut data = String::new();
    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if let Ok(value) = serde_json::from_str::<Value>(&data) {
                out.push(value);
            }
            data.clear();
            continue;
        }
        if let Some(chunk) = line.strip_prefix("data:") {
            data.push_str(chunk.trim_start());
        }
    }
    if let Ok(value) = serde_json::from_str::<Value>(&data) {
        out.push(value);
    }
    out
}

#[cfg(test)]
#[path = "upstream_tests.rs"]
mod upstream_tests;
