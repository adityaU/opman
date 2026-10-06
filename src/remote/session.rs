//! One remote server's login state and request dispatch.
//!
//! The bearer is cached per remote and minted by the remote's own `POST /api/auth/login`.
//! A 401 means the remote restarted (its JWT secret is random per run) or the token aged
//! out, so a replayable request logs in once more and is sent again. A streamed request
//! body has already been consumed by then, so its 401 goes back to the caller as-is.

use std::time::Duration;

use axum::body::Bytes;
use axum::http::{HeaderMap, Method, StatusCode};
use serde::Deserialize;
use tokio::sync::Mutex;

use super::types::{Credentials, RemoteError, ServerRecord, ServerStatus, ServerUrl};
use crate::mcp_oauth::Secret;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on a status probe or an add/edit validation, so one dead remote cannot stall
/// `GET /api/servers`.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// A request on its way to a remote.
pub struct Outbound {
    pub method: Method,
    pub url: String,
    pub headers: HeaderMap,
    pub body: OutboundBody,
}

pub enum OutboundBody {
    /// Fully buffered (or empty): can be sent again after a re-login.
    Replayable(Bytes),
    /// Streamed through as it arrives: sent exactly once.
    Streamed(reqwest::Body),
}

#[derive(Deserialize)]
struct LoginResponse {
    token: Secret,
}

pub struct Remote {
    pub record: ServerRecord,
    /// Held across a login so concurrent requests wait for one login instead of racing.
    token: Mutex<Option<Secret>>,
}

impl Remote {
    pub fn new(record: ServerRecord) -> Self {
        Self {
            record,
            token: Mutex::new(None),
        }
    }

    /// The bearer to send, logging in when none is cached. `None` for an open remote.
    pub async fn bearer(&self, http: &reqwest::Client) -> Result<Option<Secret>, RemoteError> {
        let Credentials::Password { username, password } = &self.record.credentials else {
            return Ok(None);
        };
        let mut slot = self.token.lock().await;
        if let Some(token) = slot.as_ref() {
            return Ok(Some(token.clone()));
        }
        let token = login(http, &self.record.url, username, password).await?;
        *slot = Some(token.clone());
        Ok(Some(token))
    }

    /// Called after `stale` drew a 401. Logs in again unless a concurrent request
    /// already replaced the token, in which case that newer token is returned.
    pub async fn relogin(
        &self,
        http: &reqwest::Client,
        stale: Option<&Secret>,
    ) -> Result<Option<Secret>, RemoteError> {
        let Credentials::Password { username, password } = &self.record.credentials else {
            return Ok(None);
        };
        let mut slot = self.token.lock().await;
        let replaced = match (slot.as_ref(), stale) {
            (Some(current), Some(old)) => current.expose() != old.expose(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if replaced {
            return Ok(slot.clone());
        }
        *slot = None;
        let token = login(http, &self.record.url, username, password).await?;
        *slot = Some(token.clone());
        Ok(Some(token))
    }

    pub async fn send(
        &self,
        http: &reqwest::Client,
        outbound: Outbound,
    ) -> Result<reqwest::Response, RemoteError> {
        let Outbound {
            method,
            url,
            headers,
            body,
        } = outbound;
        let bearer = self.bearer(http).await?;
        let bytes = match body {
            OutboundBody::Streamed(stream) => {
                return dispatch(http, method, &url, headers, bearer.as_ref(), stream).await;
            }
            OutboundBody::Replayable(bytes) => bytes,
        };
        let first = dispatch(
            http,
            method.clone(),
            &url,
            headers.clone(),
            bearer.as_ref(),
            bytes.clone().into(),
        )
        .await?;
        let open = matches!(self.record.credentials, Credentials::Open);
        if first.status() != StatusCode::UNAUTHORIZED || open {
            return Ok(first);
        }
        let fresh = self.relogin(http, bearer.as_ref()).await?;
        dispatch(http, method, &url, headers, fresh.as_ref(), bytes.into()).await
    }

    /// Log in if needed and confirm the token with `GET /api/auth/verify`.
    pub async fn check(&self, http: &reqwest::Client) -> Result<(), RemoteError> {
        let outbound = Outbound {
            method: Method::GET,
            url: self.record.url.api("auth/verify", None),
            headers: HeaderMap::new(),
            body: OutboundBody::Replayable(Bytes::new()),
        };
        let attempt = tokio::time::timeout(CHECK_TIMEOUT, self.send(http, outbound));
        let response = attempt
            .await
            .map_err(|_| RemoteError::Unreachable("timed out".to_owned()))??;
        match response.status() {
            s if s.is_success() => Ok(()),
            StatusCode::UNAUTHORIZED => Err(RemoteError::AuthFailed),
            s => Err(RemoteError::Unreachable(format!("auth check answered {s}"))),
        }
    }

    pub async fn probe(&self, http: &reqwest::Client) -> ServerStatus {
        match self.check(http).await {
            Ok(()) => ServerStatus::Ok,
            Err(e) => e.status(),
        }
    }
}

async fn dispatch(
    http: &reqwest::Client,
    method: Method,
    url: &str,
    headers: HeaderMap,
    bearer: Option<&Secret>,
    body: reqwest::Body,
) -> Result<reqwest::Response, RemoteError> {
    let mut request = http.request(method, url).headers(headers).body(body);
    if let Some(token) = bearer {
        request = request.bearer_auth(token.expose());
    }
    request
        .send()
        .await
        .map_err(|e| RemoteError::Unreachable(e.to_string()))
}

async fn login(
    http: &reqwest::Client,
    url: &ServerUrl,
    username: &str,
    password: &Secret,
) -> Result<Secret, RemoteError> {
    let response = http
        .post(url.api("auth/login", None))
        .timeout(LOGIN_TIMEOUT)
        .json(&serde_json::json!({ "username": username, "password": password.expose() }))
        .send()
        .await
        .map_err(|e| RemoteError::Unreachable(e.to_string()))?;
    match response.status() {
        StatusCode::OK => response
            .json::<LoginResponse>()
            .await
            .map(|body| body.token)
            .map_err(|e| RemoteError::Unreachable(format!("unexpected login reply: {e}"))),
        StatusCode::UNAUTHORIZED => Err(RemoteError::AuthFailed),
        status => Err(RemoteError::Unreachable(format!("login answered {status}"))),
    }
}
