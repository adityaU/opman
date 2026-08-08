//! Sending a message to an opencode session, plus the pure helpers that shape
//! the request body and interpret the response.

impl super::WebStateHandle {
    /// Send a message to a session via the opencode proxy.
    ///
    /// An optional `ModelRef` can be provided to override the model for this message.
    /// Returns `Ok(())` on success, or `Err(description)` on failure.
    pub(super) async fn send_to_session(
        &self,
        session_id: &str,
        project_index: &usize,
        message: &str,
        model: Option<&crate::web::types::ModelRef>,
    ) -> Result<(), String> {
        let dir = {
            let state = self.inner.read().await;
            state
                .projects
                .get(*project_index)
                .map(|p| p.path.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        if dir.is_empty() {
            tracing::warn!(
                session_id = %session_id,
                "Cannot send message: no project directory found"
            );
            return Err("No project directory found".to_string());
        }

        let base = crate::app::base_url().to_string();
        let url = format!("{}/session/{}/message", base, session_id);

        let body = build_send_message_body(message, model);

        let client = reqwest::Client::new();
        match client
            .post(&url)
            .header("x-opencode-directory", &dir)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    tracing::debug!(
                        session_id = %session_id,
                        "Message sent successfully"
                    );
                } else {
                    let detail = resp.text().await.unwrap_or_default();
                    tracing::warn!(
                        session_id = %session_id,
                        status = %status,
                        detail = %detail,
                        "Message rejected by upstream"
                    );
                }
                map_send_status(status)
            }
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to send message"
                );
                Err(format!("Failed to send message: {e}"))
            }
        }
    }
}

// ── Pure helpers (extracted for testability) ────────────────────────

/// Build the JSON body sent to `POST /session/{id}/message`.
///
/// Includes an optional `model` override object when a `ModelRef` is provided.
pub(crate) fn build_send_message_body(
    message: &str,
    model: Option<&crate::web::types::ModelRef>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "parts": [{ "type": "text", "text": message }]
    });
    if let Some(model_ref) = model {
        body["model"] = serde_json::json!({
            "providerID": model_ref.provider_id,
            "modelID": model_ref.model_id,
        });
    }
    body
}

/// Map an upstream HTTP status for a message-send into the handler result.
pub(crate) fn map_send_status(status: reqwest::StatusCode) -> Result<(), String> {
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("Upstream rejected message: HTTP {status}"))
    }
}

/// Extract a session ID from a `POST /session` response body.
pub(crate) fn parse_session_id_from_body(body: &serde_json::Value) -> Result<String, String> {
    body.get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "No session ID in response".to_string())
}

/// Extract text content from a message Value.
pub(super) fn extract_message_text(msg: &serde_json::Value) -> String {
    // Try parts array first
    if let Some(parts) = msg.pointer("/info/parts").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = parts
            .iter()
            .filter_map(|p| {
                if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                    p.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }

    // Fallback: try content array
    if let Some(content) = msg.pointer("/info/content").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = content
            .iter()
            .filter_map(|c| {
                if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                    c.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .collect();
        return texts.join("\n");
    }

    String::new()
}

#[cfg(test)]
#[path = "assistant_helpers_tests.rs"]
mod assistant_helpers_tests;
