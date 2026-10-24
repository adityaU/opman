//! Claim, inject and finish a session's one-time opening instructions.

use crate::web::session_instructions;
use crate::web::types::ServerState;
use crate::web::web_state::WebStateHandle;

/// A delivery reservation. Dropping an unfinished reservation releases it, so
/// cancellation and every early-return path leave the first turn retryable.
pub(super) struct DeliveryClaim {
    web_state: WebStateHandle,
    session_id: String,
    finished: bool,
}

impl DeliveryClaim {
    fn new(web_state: WebStateHandle, session_id: &str) -> Self {
        Self {
            web_state,
            session_id: session_id.to_string(),
            finished: false,
        }
    }

    pub(super) async fn finish(mut self, target_session: Option<&str>, accepted: bool) {
        if accepted {
            self.web_state
                .mark_instructions_delivered(&self.session_id)
                .await;
            if let Some(target) = target_session.filter(|target| *target != self.session_id) {
                self.web_state.mark_instructions_delivered(target).await;
            }
        } else {
            self.web_state
                .release_instructions_delivery(&self.session_id)
                .await;
        }
        self.finished = true;
    }
}

impl Drop for DeliveryClaim {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let web_state = self.web_state.clone();
        let session_id = self.session_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                web_state.release_instructions_delivery(&session_id).await;
            });
        }
    }
}

/// Prepare a first turn and return its claim until the send is accepted.
pub(super) async fn prepare(
    state: &ServerState,
    session_id: &str,
    directory: &str,
    parts: &mut Vec<serde_json::Value>,
    force: bool,
) -> Option<DeliveryClaim> {
    if !force && state.web_state.instructions_delivered(session_id).await {
        return None;
    }

    if !force {
        let history = state
            .runner_registry
            .messages(session_id, directory)
            .await
            .unwrap_or(serde_json::Value::Null);
        if session_instructions::has_user_turn(&history) {
            state
                .web_state
                .mark_instructions_delivered(session_id)
                .await;
            return None;
        }
        if !state
            .web_state
            .claim_instructions_delivery(session_id)
            .await
        {
            return None;
        }
    }

    let claim = (!force).then(|| DeliveryClaim::new(state.web_state.clone(), session_id));

    let instructions = state
        .web_state
        .list_active_memory(
            Some(state.web_state.active_project_index().await),
            Some(session_id),
        )
        .await;
    if instructions.is_empty() {
        if let Some(claim) = claim {
            claim.finish(None, false).await;
        }
        return None;
    }

    if !session_instructions::apply_to_parts(parts, &instructions) {
        if parts.is_empty() {
            parts.push(serde_json::json!({
                "type": "text",
                "text": session_instructions::wrap("", &instructions),
            }));
            return claim;
        }
        let has_text = parts.iter().any(|part| {
            part.get("type")
                .and_then(serde_json::Value::as_str)
                .map_or(true, |kind| kind == "text")
                && part
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
        });
        if !has_text {
            parts.insert(
                0,
                serde_json::json!({
                    "type": "text",
                    "text": session_instructions::wrap("", &instructions),
                }),
            );
        }
    }
    claim
}
