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

    /// Fetch the current status of all sessions from the server.
    ///
    /// Makes a `GET /session/status` request. Returns a map of session IDs to
    /// their status. Sessions that are idle are absent from the map; only busy
    /// or retry sessions appear.
    pub async fn fetch_session_status(
        &self,
        base_url: &str,
        project_dir: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let url = format!("{}/session/status", base_url);
        debug!(url, project_dir, "Fetching session status");

        let response = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch session status from opencode server")?;

        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse session status response")?;

        // The response is a Record<sessionID, { type: "busy"|"retry"|"idle" }>
        // Only busy/retry sessions are present; idle ones are absent.
        let mut status_map = std::collections::HashMap::new();
        if let Some(obj) = body.as_object() {
            for (session_id, status) in obj {
                if let Some(status_type) = status.get("type").and_then(|t| t.as_str()) {
                    status_map.insert(session_id.clone(), status_type.to_string());
                }
            }
        }

        Ok(status_map)
    }

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

    /// Send a message to a session via the OpenCode API.
    ///
    /// Uses `POST /session/{id}/message` with a text part payload.
    /// The response is streamed by the server; we discard it (fire-and-forget).
    #[allow(dead_code)]
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

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "system": "true",
                "parts": [{ "type": "text", "text": text }]
            }))
            .send()
            .await
            .context("Failed to send async system message to opencode session")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "System message rejected by server: HTTP {} — {}",
                status,
                body
            );
        }

        Ok(())
    }

    /// Fetch messages for a session, returning only user-role messages.
    ///
    /// Uses `GET /session/{id}/message` with the project directory header.
    /// Returns a list of `SessionMessage` with role and text.
    pub async fn fetch_session_messages(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<Vec<crate::app::SessionMessage>> {
        let url = format!("{}/session/{}/message", base_url, session_id);
        debug!(url, session_id, "Fetching session messages for watcher");

        let response = self
            .client
            .get(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch session messages")?;

        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse session messages response")?;

        let mut messages = Vec::new();
        // The response is an array of `{ info: { role, ... }, parts: [...] }`.
        // It may also be an object with message IDs as keys.
        let items: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
            arr.iter().collect()
        } else if let Some(obj) = body.as_object() {
            obj.values().collect()
        } else {
            vec![]
        };

        for item in items {
            // The role lives under item.info.role (not item.role).
            let info = item.get("info");
            let role = info
                .and_then(|i| i.get("role"))
                .and_then(|r| r.as_str())
                // Fallback: try top-level role for older API versions.
                .or_else(|| item.get("role").and_then(|r| r.as_str()))
                .unwrap_or("")
                .to_string();
            if role != "user" {
                continue;
            }
            // Parts are at item.parts (top-level, same level as info).
            let text = if let Some(parts) = item.get("parts").and_then(|p| p.as_array()) {
                parts
                    .iter()
                    .filter_map(|p| {
                        // Only extract text from "text" type parts.
                        let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if ptype == "text" || ptype.is_empty() {
                            p.get("text").and_then(|t| t.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                t.to_string()
            } else if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
                content.to_string()
            } else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            messages.push(crate::app::SessionMessage { role, text });
        }

        Ok(messages)
    }

    /// Abort an active session, stopping any ongoing AI processing.
    ///
    /// Uses `POST /session/{id}/abort`. The server calls `SessionPrompt.cancel()`
    /// internally, which interrupts the running LLM generation and tool execution.
    pub async fn abort_session(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<()> {
        let url = format!("{}/session/{}/abort", base_url, session_id);
        debug!(url, session_id, "Aborting session");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send abort request to opencode session")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Session abort rejected by server: HTTP {} — {}",
                status,
                body
            );
        }

        Ok(())
    }

    /// Reply to a permission request from the AI agent.
    ///
    /// Uses `POST /permission/{id}/reply` with `{reply: "once"|"always"|"reject"}`.
    pub async fn reply_permission(
        &self,
        base_url: &str,
        project_dir: &str,
        request_id: &str,
        reply: &str,
    ) -> Result<()> {
        let url = format!("{}/permission/{}/reply", base_url, request_id);
        debug!(url, request_id, reply, "Replying to permission request");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "reply": reply }))
            .send()
            .await
            .context("Failed to reply to permission request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Permission reply rejected by server: HTTP {} — {}",
                status,
                body
            );
        }

        Ok(())
    }

    /// Reply to a question from the AI agent.
    ///
    /// Uses `POST /question/{id}/reply` with `{answers: string[][]}`.
    /// Each inner array contains the selected option labels or custom text.
    pub async fn reply_question(
        &self,
        base_url: &str,
        project_dir: &str,
        request_id: &str,
        answers: &[Vec<String>],
    ) -> Result<()> {
        let url = format!("{}/question/{}/reply", base_url, request_id);
        debug!(url, request_id, "Replying to question");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "answers": answers }))
            .send()
            .await
            .context("Failed to reply to question")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Question reply rejected by server: HTTP {} — {}",
                status,
                body
            );
        }

        Ok(())
    }

    /// Reject/dismiss a question from the AI agent.
    ///
    /// Uses `POST /question/{id}/reject`.
    pub async fn reject_question(
        &self,
        base_url: &str,
        project_dir: &str,
        request_id: &str,
    ) -> Result<()> {
        let url = format!("{}/question/{}/reject", base_url, request_id);
        debug!(url, request_id, "Rejecting question");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to reject question")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Question reject failed: HTTP {} — {}", status, body);
        }

        Ok(())
    }
    /// Execute a slash command on a session via the OpenCode command API.
    ///
    /// Uses `POST /session/{id}/command` with `{ command, arguments, model? }`.
    /// Returns the server's JSON response body (shape varies by command).
    pub async fn execute_session_command(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
        command: &str,
        arguments: &str,
        model: Option<&str>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/session/{}/command", base_url, session_id);
        debug!(
            url,
            session_id, command, arguments, "Executing session command"
        );

        let mut body = serde_json::json!({
            "command": command,
            "arguments": arguments,
        });
        if let Some(m) = model {
            body["model"] = serde_json::Value::String(m.to_string());
        }

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to execute session command")?;

        let status = resp.status();
        let response_body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);

        if !status.is_success() {
            let err_msg = response_body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!(
                "Session command '{}' rejected: HTTP {} — {}",
                command,
                status,
                err_msg
            );
        }

        Ok(response_body)
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
}
