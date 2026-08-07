//! opman as the OAuth client for MCP servers, so a credential is obtained once here and
//! never handed to a runner.
//!
//! Implements the subset of the MCP authorization spec a client must: protected-resource
//! metadata discovery (RFC 9728), authorization-server metadata over *both* RFC 8414 and
//! OpenID Connect Discovery, dynamic client registration (RFC 7591) with a pre-registered
//! fallback, PKCE S256, the `resource` parameter (RFC 8707) on both legs, and RFC 9207
//! `iss` validation.

pub mod callback;
pub mod discovery;
pub mod flow;
pub mod store;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use store::{Credential, TokenRecord, TokenStore};

/// A secret that will not print itself. This codebase logs heavily; a token reaching a
/// log through a derived `Debug` is the failure mode this exists to prevent.
#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// A validated registry name, used as a filename and a proxy argument.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServerName(String);

impl ServerName {
    pub fn parse(raw: &str) -> Result<Self, OAuthError> {
        let name = raw.trim();
        let ok = !name.is_empty()
            && name.len() <= 64
            && name != "."
            && name != ".."
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !ok {
            return Err(OAuthError::BadServerName(raw.to_string()));
        }
        Ok(Self(name.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
pub enum OAuthError {
    NoConfigDir,
    BadServerName(String),
    /// No usable credential is stored. Never resolved by opening a browser from a proxy
    /// child — a background process must not ambush the user with a window.
    LoginRequired,
    /// The server is not declared in `mcp.json`.
    NotConfigured(String),
    /// The server needs a pre-registered client id and none is configured.
    NeedsPreRegistration(String),
    Discovery(String),
    /// RFC 9207: the `iss` in the authorization response did not match the recorded
    /// issuer. Deliberately carries no server-supplied text, because the spec forbids
    /// acting on or displaying the error fields when this happens.
    IssuerMismatch,
    StateMismatch,
    Denied(String),
    Http(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfigDir => f.write_str("no config directory available"),
            Self::BadServerName(name) => write!(f, "invalid MCP server name '{name}'"),
            Self::LoginRequired => f.write_str("not authenticated"),
            Self::NotConfigured(name) => {
                write!(f, "MCP server '{name}' is not configured in mcp.json")
            }
            Self::NeedsPreRegistration(name) => write!(
                f,
                "'{name}' does not support dynamic client registration; set clientId and \
                 callbackPort in mcp.json"
            ),
            Self::Discovery(detail) => write!(f, "OAuth discovery failed: {detail}"),
            Self::IssuerMismatch => f.write_str("authorization response came from the wrong issuer"),
            Self::StateMismatch => f.write_str("authorization response state did not match"),
            Self::Denied(reason) => write!(f, "authorization denied: {reason}"),
            Self::Http(detail) => write!(f, "HTTP error: {detail}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OAuthError {}

/// A non-interactive access token for `name`, refreshing if needed.
///
/// This is what the proxy calls per request. It never opens a browser: a missing
/// credential is [`OAuthError::LoginRequired`], which the proxy turns into an actionable
/// message for the model rather than a surprise window.
pub async fn access_token(
    http: &reqwest::Client,
    store: &TokenStore,
    name: &ServerName,
) -> Result<Secret, OAuthError> {
    let now = store::now_secs();
    let record = store.load(name).ok_or(OAuthError::LoginRequired)?;
    match record.credential(now) {
        Credential::Fresh(token) => Ok(token.clone()),
        Credential::Refreshable(_) => {
            let refreshed = store
                .refresh_once(name, now, |record| flow::refresh(http, record))
                .await?;
            Ok(refreshed.access_token)
        }
        Credential::Unusable => Err(OAuthError::LoginRequired),
    }
}

/// Refresh even when the record looks fresh — used after a 401 on a token we believed was
/// good, which is the case a plain expiry check cannot catch.
pub async fn force_refresh(
    http: &reqwest::Client,
    store: &TokenStore,
    name: &ServerName,
) -> Result<Secret, OAuthError> {
    let record = store.load(name).ok_or(OAuthError::LoginRequired)?;
    if record.refresh_token.is_none() {
        return Err(OAuthError::LoginRequired);
    }
    let refreshed = flow::refresh(http, record).await?;
    store.save(name, &refreshed)?;
    Ok(refreshed.access_token)
}

pub fn logout(store: &TokenStore, name: &ServerName) -> Result<(), OAuthError> {
    store.delete(name)
}

/// Whether a credential is currently usable, for the settings page's status column.
pub fn is_authenticated(store: &TokenStore, name: &ServerName) -> bool {
    store
        .load(name)
        .map(|record| !matches!(record.credential(store::now_secs()), Credential::Unusable))
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
