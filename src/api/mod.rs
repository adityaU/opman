mod interactions;
mod messages;
mod queries;
mod sessions;

use reqwest::Client;
use serde::Deserialize;

/// Client for communicating with a running opencode server's REST API.
pub struct ApiClient {
    /// Underlying HTTP client.
    client: Client,
}

/// Basic project/server info returned from the opencode API.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ProjectInfo {
    /// The project directory being served.
    #[serde(default)]
    pub directory: String,
    /// Server version string.
    #[serde(default)]
    pub version: String,
}

impl ApiClient {
    /// Create a new API client with a default reqwest Client.
    ///
    /// Prefer `with_client()` to share a single connection pool.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Create an API client backed by a shared reqwest Client.
    ///
    /// Reuses TCP connections and avoids per-call connection-pool overhead.
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }
}
