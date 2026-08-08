//! The loopback descriptor `~/.config/opman/internal.json`.
//!
//! opman's stdio MCP servers run as children of the *runner*, not of opman, so they reach
//! the web server the same way any other local client would: over HTTP on the loopback
//! URL, authenticated with a shared token. The web server writes both at startup.
//!
//! Owning the path here rather than in each consumer is what lets `Presence` gate a server
//! on "the web server is up" without re-deriving where that fact is written down.

use std::path::{Path, PathBuf};

/// Where the web server publishes its loopback URL and token.
pub fn descriptor_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("opman").join("internal.json"))
}

/// True once the web server has published a descriptor, i.e. `/internal/*` is dialable.
pub fn is_available() -> bool {
    descriptor_path().is_some_and(|path| path.is_file())
}

/// A resolved loopback client: base URL, shared token, and a connection-pooled agent.
#[derive(Clone)]
pub struct Loopback {
    pub url: String,
    pub token: String,
    pub client: reqwest::Client,
}

impl Loopback {
    pub fn load() -> Option<Self> {
        Self::load_from(&descriptor_path()?)
    }

    /// Parse a descriptor from a specific path. Separate from [`Self::load`] so the
    /// parsing is testable without depending on the real config directory. `None` when the
    /// file is missing, malformed, or lacks the required `url`/`token` string fields.
    pub fn load_from(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let value: serde_json::Value = serde_json::from_str(&content).ok()?;
        Some(Self {
            url: value.get("url")?.as_str()?.to_string(),
            token: value.get("token")?.as_str()?.to_string(),
            client: reqwest::Client::new(),
        })
    }

    /// A request builder for `path` with the shared token already attached.
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}{path}", self.url))
            .header("x-internal-token", &self.token)
    }

    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .get(format!("{}{path}", self.url))
            .header("x-internal-token", &self.token)
    }
}

#[cfg(test)]
#[path = "loopback_tests.rs"]
mod loopback_tests;
