//! Value types for remote servers: validated ids and URLs, stored records, errors.

use std::fmt;

use serde::Serialize;

use crate::mcp_oauth::Secret;

/// The id the home server answers to. Never handed out to a remote.
pub const HOME_ID: &str = "home";

const MAX_ID_LEN: usize = 32;

/// A short URL-safe slug naming one remote server: `[a-z0-9-]`, no leading or trailing
/// dash, never `home`. It is a path segment in `/remote/{id}/`, so it is validated once
/// here rather than escaped at every use.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ServerId(String);

impl ServerId {
    pub fn parse(raw: &str) -> Option<Self> {
        let valid = !raw.is_empty()
            && raw.len() <= MAX_ID_LEN
            && raw != HOME_ID
            && !raw.starts_with('-')
            && !raw.ends_with('-')
            && raw
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        valid.then(|| Self(raw.to_owned()))
    }

    /// Slug `name` and suffix `-2`, `-3`, … until `taken` says it is free.
    pub fn derive(name: &str, taken: impl Fn(&ServerId) -> bool) -> Self {
        let base = slugify(name);
        let first = Self::parse(&base).unwrap_or_else(|| Self(format!("{base}-1")));
        if !taken(&first) {
            return first;
        }
        let mut n = 2u32;
        loop {
            let suffix = format!("-{n}");
            let stem = &base[..base.len().min(MAX_ID_LEN - suffix.len())];
            let candidate = Self(format!("{}{suffix}", stem.trim_end_matches('-')));
            if !taken(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(MAX_ID_LEN - 3);
    let slug = slug.trim_end_matches('-');
    match slug {
        "" => "server".to_owned(),
        s => s.to_owned(),
    }
}

/// An `http`/`https` base URL with no trailing slash, query or fragment. A path is kept,
/// so a remote behind a reverse proxy at `https://host/opman` works.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ServerUrl(String);

impl ServerUrl {
    pub fn parse(raw: &str) -> Result<Self, RemoteError> {
        let invalid = |why: &str| RemoteError::InvalidUrl(format!("{}: {why}", raw.trim()));
        let url = url::Url::parse(raw.trim()).map_err(|e| invalid(&e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(invalid("must start with http:// or https://"));
        }
        if url.host_str().is_none_or(str::is_empty) {
            return Err(invalid("missing host"));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(invalid("must not have a query or fragment"));
        }
        Ok(Self(url.as_str().trim_end_matches('/').to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `{base}/api/{rest}` plus `query` when there is one.
    pub fn api(&self, rest: &str, query: Option<&str>) -> String {
        let rest = rest.trim_start_matches('/');
        match query.filter(|q| !q.is_empty()) {
            Some(q) => format!("{}/api/{rest}?{q}", self.0),
            None => format!("{}/api/{rest}", self.0),
        }
    }

    /// The WebSocket URL for `path` (which starts with `/`): `http` → `ws`, `https` → `wss`.
    pub fn ws(&self, path: &str) -> String {
        let (scheme, rest) = match self.0.strip_prefix("https://") {
            Some(rest) => ("wss://", rest),
            None => ("ws://", self.0.trim_start_matches("http://")),
        };
        format!("{scheme}{rest}/{}", path.trim_start_matches('/'))
    }
}

/// How home proves itself to a remote.
#[derive(Clone, Debug)]
pub enum Credentials {
    /// The remote runs without auth; requests go out with no bearer.
    Open,
    Password { username: String, password: Secret },
}

/// One configured remote, as held in memory. The password never leaves this type
/// except on the way to the remote's login endpoint or into `servers.json`.
#[derive(Clone, Debug)]
pub struct ServerRecord {
    pub id: ServerId,
    pub name: String,
    pub url: ServerUrl,
    pub credentials: Credentials,
}

/// What other subsystems may know about a remote: no credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSnapshot {
    pub id: ServerId,
    pub name: String,
    pub url: ServerUrl,
}

impl From<&ServerRecord> for ServerSnapshot {
    fn from(record: &ServerRecord) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            url: record.url.clone(),
        }
    }
}

/// Reachability of one server, as shown in the server selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Ok,
    Unreachable,
    AuthFailed,
}

#[derive(Debug)]
pub enum RemoteError {
    UnknownServer,
    InvalidName,
    InvalidUrl(String),
    /// Transport failure, timeout, or an unexpected status from the remote.
    Unreachable(String),
    /// The remote rejected the stored credentials.
    AuthFailed,
    Store(std::io::Error),
    WebSocket(String),
}

impl RemoteError {
    pub fn status(&self) -> ServerStatus {
        match self {
            Self::AuthFailed => ServerStatus::AuthFailed,
            _ => ServerStatus::Unreachable,
        }
    }
}

impl fmt::Display for RemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownServer => f.write_str("unknown server"),
            Self::InvalidName => f.write_str("server name must not be empty"),
            Self::InvalidUrl(why) => write!(f, "invalid server URL {why}"),
            Self::Unreachable(why) => write!(f, "server unreachable: {why}"),
            Self::AuthFailed => f.write_str("the server rejected the username or password"),
            Self::Store(e) => write!(f, "could not save servers.json: {e}"),
            Self::WebSocket(why) => write!(f, "websocket to server failed: {why}"),
        }
    }
}

impl std::error::Error for RemoteError {}

#[cfg(test)]
#[path = "types_tests.rs"]
mod types_tests;
