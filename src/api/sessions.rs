use anyhow::{Context, Result};
use tracing::debug;

use super::ApiClient;
use crate::app::SessionInfo;

impl ApiClient {
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

    /// Revert the last message in a session, undoing its effects and restoring
    /// the previous state.
    ///
    /// Uses `POST /session/{id}/revert` with `{ messageID }` from the session's
    /// current revert pointer. If no revert pointer exists, reverts the last
    /// assistant message.
    pub async fn revert_session(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<()> {
        // First fetch the session to get the last message ID to revert.
        let session_url = format!("{}/session/{}", base_url, session_id);
        let session_resp = self
            .client
            .get(&session_url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch session for revert")?;

        let session: serde_json::Value = session_resp
            .json()
            .await
            .context("Failed to parse session response")?;

        // Get the last message ID from the session messages.
        let messages_url = format!("{}/session/{}/message", base_url, session_id);
        let msgs_resp = self
            .client
            .get(&messages_url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch session messages for revert")?;

        let messages: serde_json::Value = msgs_resp
            .json()
            .await
            .context("Failed to parse messages response")?;

        // Find the last user message ID to revert from.
        let message_id = messages
            .as_array()
            .and_then(|msgs| {
                msgs.iter().rev().find_map(|m| {
                    let info = m.get("info")?;
                    if info.get("role")?.as_str()? == "user" {
                        info.get("id")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                session
                    .get("revert")
                    .and_then(|r| r.get("messageID"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            });

        let message_id = match message_id {
            Some(id) => id,
            None => anyhow::bail!("No message found to revert"),
        };

        let url = format!("{}/session/{}/revert", base_url, session_id);
        debug!(url, session_id, message_id, "Reverting session");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "messageID": message_id }))
            .send()
            .await
            .context("Failed to send revert request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Session revert rejected: HTTP {} — {}", status, body);
        }

        Ok(())
    }

    /// Restore all previously reverted messages in a session (redo).
    ///
    /// Uses `POST /session/{id}/unrevert`.
    pub async fn unrevert_session(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<()> {
        let url = format!("{}/session/{}/unrevert", base_url, session_id);
        debug!(url, session_id, "Unreverting session");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send unrevert request")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Session unrevert rejected: HTTP {} — {}", status, body);
        }

        Ok(())
    }

    /// Share a session, creating a shareable link.
    ///
    /// Uses `POST /session/{id}/share`.
    pub async fn share_session(
        &self,
        base_url: &str,
        project_dir: &str,
        session_id: &str,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/session/{}/share", base_url, session_id);
        debug!(url, session_id, "Sharing session");

        let resp = self
            .client
            .post(&url)
            .header("x-opencode-directory", project_dir)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to send share request")?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);

        if !status.is_success() {
            let err = body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Session share rejected: HTTP {} — {}", status, err);
        }

        Ok(body)
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod sessions_tests;
