use anyhow::{Context, Result};
use tracing::debug;

use super::{ApiClient, ProjectInfo};

impl ApiClient {
    /// Fetch basic project info from a running opencode server.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub async fn health_check(&self, base_url: &str) -> Result<bool> {
        let url = format!("{}/health", base_url);

        match self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Fetch the context window size for a given model from the provider API.
    ///
    /// Makes a `GET /provider` request and searches all providers for the model,
    /// returning its context window limit (in tokens).
    #[allow(dead_code)]
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

    /// Fetch all available providers and their models from the OpenCode server.
    ///
    /// Uses `GET /provider` with the project directory header.
    /// Returns the raw JSON response (array of provider objects with models).
    pub async fn fetch_providers(
        &self,
        base_url: &str,
        project_dir: &str,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/provider", base_url);
        debug!(url, project_dir, "Fetching providers and models");

        let resp = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch providers from opencode server")?;

        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse provider response")?;

        Ok(body)
    }

    /// List all available commands (built-in + custom) from the OpenCode server.
    ///
    /// Uses `GET /command` with the project directory header.
    /// Returns the raw JSON response (array of command definitions).
    #[allow(dead_code)]
    pub async fn list_commands(
        &self,
        base_url: &str,
        project_dir: &str,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/command", base_url);
        debug!(url, project_dir, "Listing available commands");

        let resp = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to list commands from opencode server")?;

        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse command list response")?;

        Ok(body)
    }
}
