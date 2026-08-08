//! Loopback-only question API, called by the `opman mcp-ask` server.
//!
//! Mounted outside `/api` so it skips the `AuthUser` extractor; the shared
//! `X-Internal-Token` written to `~/.config/opman/internal.json` is the check instead, same
//! as the Kanban internal routes.
//!
//! The request *is* the wait. It is answered only once the user has replied, so the agent
//! sits in a tool call rather than ending its turn on an unanswered question — which is
//! the whole point of routing questions through MCP instead of through prose.

use std::time::Duration;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::error::{WebError, WebResult};
use super::super::types::ServerState;

/// How long a question waits for a human before it is treated as unanswered. Matches the
/// ACP engine's permission ceiling — a question is the same kind of wait.
const ASK_TIMEOUT: Duration = Duration::from_secs(3600);

#[derive(Deserialize)]
pub struct AskRequest {
    /// The session the asker belongs to. Empty when its runner's MCP config is
    /// process-wide and `${session}` had nothing to resolve against.
    #[serde(default, rename = "sessionID")]
    session_id: String,
    #[serde(default)]
    directory: String,
    #[serde(default)]
    questions: Vec<Value>,
}

/// POST /internal/ask — raise a question card and block until it is answered.
pub async fn internal_ask(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<AskRequest>,
) -> WebResult<Json<Value>> {
    check_internal_token(&state, &headers)?;
    if request.questions.is_empty() {
        return Err(WebError::BadRequest("no questions".into()));
    }
    let session_id = resolve_session(&state, &request).await;
    let id = rand_ask_id();
    let rx = state.ask_pending.register(&id, &session_id);

    state
        .web_state
        .publish_event(
            &json!({
                "type": "question.asked",
                "properties": {
                    "id": id,
                    "sessionID": session_id,
                    "questions": request.questions,
                },
            }),
            &request.directory,
        )
        .await;

    // Clears the card however this ends — answered, dismissed, timed out, or the asker
    // hanging up because its turn was cancelled. Without the guard, a dropped request
    // leaves a card on screen that resolves nothing when clicked.
    let _guard = Clear::new(&state, &id, &session_id, &request.directory);
    let answers = match tokio::time::timeout(ASK_TIMEOUT, rx).await {
        Ok(Ok(answers)) => answers,
        // Dismissed (the waiter was dropped) or aged out. Either way the asker is told
        // nobody answered, and decides its own default.
        _ => Vec::new(),
    };
    Ok(Json(json!({ "answers": answers })))
}

/// Emits `question.replied` on every exit path, including the cancellation that axum
/// performs when the client disconnects — the one case no explicit branch can cover.
struct Clear {
    state: ServerState,
    id: String,
    session_id: String,
    directory: String,
}

impl Clear {
    fn new(state: &ServerState, id: &str, session_id: &str, directory: &str) -> Self {
        Self {
            state: state.clone(),
            id: id.to_string(),
            session_id: session_id.to_string(),
            directory: directory.to_string(),
        }
    }
}

impl Drop for Clear {
    fn drop(&mut self) {
        // Retire the waiter first: a card cleared while still registered would let a late
        // reply resolve a question the asker has already given up on.
        self.state.ask_pending.dismiss(&self.id);
        let state = self.state.clone();
        let event = json!({
            "type": "question.replied",
            "properties": { "id": self.id, "requestID": self.id, "sessionID": self.session_id },
        });
        let directory = std::mem::take(&mut self.directory);
        tokio::spawn(async move {
            state.web_state.publish_event(&event, &directory).await;
        });
    }
}

/// The session to attribute the card to: what the asker was told, else the newest session
/// in its project directory.
async fn resolve_session(state: &ServerState, request: &AskRequest) -> String {
    if !request.session_id.is_empty() {
        return request.session_id.clone();
    }
    state
        .web_state
        .newest_session_in(&request.directory)
        .await
        .unwrap_or_default()
}

fn check_internal_token(state: &ServerState, headers: &HeaderMap) -> WebResult<()> {
    let provided = headers
        .get("x-internal-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !state.internal_token.is_empty() && provided == state.internal_token {
        return Ok(());
    }
    Err(WebError::Unauthorized)
}

fn rand_ask_id() -> String {
    let n: u128 = rand::random();
    format!("qst_{n:032x}")
}

#[cfg(test)]
#[path = "ask_internal_tests.rs"]
mod ask_internal_tests;
