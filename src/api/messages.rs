use anyhow::{Context, Result};
use tracing::debug;

use super::ApiClient;

impl ApiClient {
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
}
