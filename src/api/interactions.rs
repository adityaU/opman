use anyhow::{Context, Result};
use tracing::debug;

use super::ApiClient;

impl ApiClient {
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
}
