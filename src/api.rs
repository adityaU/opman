use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

use crate::app::SessionInfo;

/// Client for communicating with a running opencode server's REST API.
pub struct ApiClient {
    /// Underlying HTTP client.
    client: Client,
}

/// Basic project/server info returned from the opencode API.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInfo {
    /// The project directory being served.
    #[serde(default)]
    pub directory: String,
    /// Server version string.
    #[serde(default)]
    pub version: String,
}

impl ApiClient {
    /// Create a new API client.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Fetch the list of sessions from a running opencode server.
    ///
    /// Makes a `GET /session` request with the `x-opencode-directory` header.
    pub async fn fetch_sessions(
        &self,
        base_url: &str,
        project_dir: &str,
    ) -> Result<Vec<SessionInfo>> {
        let url = format!("{}/session", base_url);
        debug!(url, project_dir, "Fetching sessions");

        let response = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch sessions from opencode server")?;

        let sessions: Vec<SessionInfo> = response
            .json()
            .await
            .context("Failed to parse session list response")?;

        Ok(sessions)
    }

    /// Fetch basic project info from a running opencode server.
    pub async fn fetch_project_info(
        &self,
        base_url: &str,
        project_dir: &str,
    ) -> Result<ProjectInfo> {
        let url = format!("{}/info", base_url);
        debug!(url, project_dir, "Fetching project info");

        let response = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch project info from opencode server")?;

        let info: ProjectInfo = response
            .json()
            .await
            .context("Failed to parse project info response")?;

        Ok(info)
    }

    /// Check if a server is reachable (simple health check).
    pub async fn health_check(&self, base_url: &str) -> Result<bool> {
        let url = format!("{}/health", base_url);

        match self.client.get(&url).header("Accept", "application/json").send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub async fn select_session(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<()> {
        let url = format!("{}/tui/select-session", base_url);
        debug!(url, session_id, "Selecting session in TUI");

        self.client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "sessionID": session_id }))
            .send()
            .await
            .context("Failed to select session")?;

        Ok(())
    }

    /// Fetch the context window size for a given model from the provider API.
    ///
    /// Makes a `GET /provider` request and searches all providers for the model,
    /// returning its context window limit (in tokens).
    pub async fn fetch_context_window(
        &self,
        base_url: &str,
        project_dir: &str,
        model_id: &str,
    ) -> Result<u64> {
        let url = format!("{}/provider", base_url);
        debug!(url, model_id, "Fetching provider info for context window");

        let response = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch provider info")?;

        let providers: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse provider response")?;

        // providers is an array of provider objects, each with a "models" record
        // where keys are model IDs and values have { "limit": { "context": N } }
        if let Some(arr) = providers.as_array() {
            for provider in arr {
                if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                    if let Some(model) = models.get(model_id) {
                        if let Some(ctx) = model
                            .get("limit")
                            .and_then(|l| l.get("context"))
                            .and_then(|c| c.as_u64())
                        {
                            return Ok(ctx);
                        }
                    }
                }
            }
        }

        // Fallback: default context window if model not found
        Ok(200_000)
    }

    pub async fn fetch_todos(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<Vec<crate::app::TodoItem>> {
        let url = format!("{}/session/{}/todo", base_url, session_id);
        debug!(url, session_id, "Fetching todos");
        let response = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch todos from opencode server")?;
        let todos = response
            .json()
            .await
            .context("Failed to parse todos response")?;
        Ok(todos)
    }

    /// Send a message to a session via the OpenCode API.
    ///
    /// Uses `POST /session/{id}/message` with a text part payload.
    /// The response is streamed by the server; we discard it (fire-and-forget).
    pub async fn send_session_message(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
        text: &str,
    ) -> Result<()> {
        let url = format!("{}/session/{}/message", base_url, session_id);
        debug!(url, session_id, "Sending context message to session");

        self.client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "parts": [{ "type": "text", "text": text }]
            }))
            .send()
            .await
            .context("Failed to send message to opencode session")?;

        Ok(())
    }

    /// Send a system message to a session asynchronously via the OpenCode API.
    ///
    /// Uses `POST /session/{id}/prompt_async` with `system: true`.
    /// The server processes it without blocking and the AI will respond to it.
    pub async fn send_system_message_async(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
        text: &str,
    ) -> Result<()> {
        let url = format!("{}/session/{}/prompt_async", base_url, session_id);
        debug!(url, session_id, "Sending async system message to session");

        self.client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "system": true,
                "parts": [{ "type": "text", "text": text }]
            }))
            .send()
            .await
            .context("Failed to send async system message to opencode session")?;

        Ok(())
    }
}
