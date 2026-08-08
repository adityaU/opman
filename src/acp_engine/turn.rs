//! Driving a turn: prompt, cancel, and settle.
//!
//! The protocol changes what these mean compared with the pipe-driven engine this replaced.
//! A follow-up mid-turn is another `session/prompt`, which agents that advertise steering
//! deliver to the running model rather than queueing. Abort is `session/cancel`, so the agent
//! unwinds and reports `stopReason: cancelled` instead of being killed and losing the turn.

use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};
use tracing::{debug, warn};

use super::attach::Prompt;
use super::AcpEngine;

/// Send a user turn, establishing the connection on first use.
///
/// Boxed rather than a plain `async fn`: a finished turn can release a queued follow-up,
/// which calls back in here, and a recursive `async fn` has no representable return type.
pub fn prompt(
    engine: Arc<AcpEngine>,
    session_id: String,
    prompt: Prompt,
) -> futures::future::BoxFuture<'static, ()> {
    Box::pin(async move {
        // Render the prompt immediately: the user should see what they sent before the
        // agent has said anything, and agents echo user messages only while replaying.
        let model = engine
            .get_session(&session_id)
            .and_then(|s| s.model)
            .unwrap_or_default();
        let emits = engine.with_transcript(&session_id, |t| {
            let mut out = Vec::new();
            // Stamp the model this turn runs under, so the message header names it.
            t.set_model(&model);
            t.user_message(&prompt, &mut out);
            out
        });
        super::render::broadcast(&engine, &session_id, emits);
        engine.set_busy(&session_id, true);

        if let Err(e) = send(&engine, &session_id, &prompt).await {
            warn!(session = %session_id, "acp prompt failed: {e}");
            super::emit_system(&engine, &session_id, "error", &e.to_string());
            engine.set_busy(&session_id, false);
        }
    })
}

async fn send(engine: &Arc<AcpEngine>, session_id: &str, prompt: &Prompt) -> Result<()> {
    let ready = engine.conns.ensure(engine, session_id).await?;
    let (peer, acp_session) = (ready.peer, ready.acp_session);
    super::conn_options::sync(engine, &peer, session_id, &acp_session).await;

    // Without steering the agent would reject or misorder a concurrent prompt, so wait for
    // the current turn to land first.
    if !ready.steering && engine.is_busy(session_id) && engine.has_inflight(session_id) {
        engine.queue_followup(session_id, prompt.clone());
        return Ok(());
    }

    // Blocks are chosen against what this agent said it accepts, so an image goes inline to
    // an agent that takes images and is named as a link to one that does not.
    let params = json!({
        "sessionId": acp_session,
        "prompt": prompt.content_blocks(ready.prompt_caps),
    });
    let engine = engine.clone();
    let session_id = session_id.to_string();
    // `session/prompt` resolves only when the whole turn is over, so awaiting it here
    // would block every later call on this connection — including the permission answers
    // the turn itself is waiting for.
    engine.mark_inflight(&session_id, true);
    tokio::spawn(async move {
        let outcome = peer.request("session/prompt", params).await;
        engine.mark_inflight(&session_id, false);
        finish(&engine, &session_id, outcome).await;
    });
    Ok(())
}

/// Settle a completed turn: report anything that went wrong, close out the transcript, and
/// release any follow-up the user typed while the agent was busy.
async fn finish(engine: &Arc<AcpEngine>, session_id: &str, outcome: Result<Value>) {
    match outcome {
        Ok(result) => {
            if let Some(usage) = result.get("usage") {
                let emits = engine.with_transcript(session_id, |t| {
                    let mut out = Vec::new();
                    t.set_usage(super::usage_tokens(usage), None, &mut out);
                    out
                });
                super::render::broadcast(engine, session_id, emits);
            }
            if let Some(reason) = stop_note(result.get("stopReason").and_then(Value::as_str)) {
                super::emit_system(engine, session_id, "warning", reason);
            }
        }
        Err(e) => {
            let message = e.to_string();
            super::emit_system(engine, session_id, "error", &message);
            // The connection is gone, not merely erroring; the next prompt must reconnect
            // and resume rather than write into a dead child.
            if message.contains("acp process exited") || message.contains("connection closed") {
                engine.conns.close(session_id).await;
            }
        }
    }
    let emits = engine.with_transcript(session_id, |t| {
        let mut out = Vec::new();
        t.finish_turn(&mut out);
        out
    });
    super::render::broadcast(engine, session_id, emits);
    engine.set_busy(session_id, false);

    if let Some(next) = engine.take_followup(session_id) {
        let engine = engine.clone();
        let session_id = session_id.to_string();
        tokio::spawn(async move { prompt(engine, session_id, next).await });
    }
}

/// Turn outcomes worth telling the user about. `end_turn` is the normal case and says
/// nothing; the rest explain why a reply looks truncated.
fn stop_note(reason: Option<&str>) -> Option<&'static str> {
    match reason? {
        "max_tokens" => Some("The turn hit the model's output limit."),
        "max_turn_requests" => Some("The turn hit the agent's request limit."),
        "refusal" => Some("The agent declined to continue this turn."),
        _ => None,
    }
}

/// Abort: ask the agent to cancel. The connection stays up, so the conversation continues
/// from wherever it stopped without a reconnect or a replay.
pub async fn abort(engine: Arc<AcpEngine>, session_id: &str) {
    let target = engine.conns.existing(session_id).await;
    if let Some((peer, acp_session)) = target {
        let params = json!({ "sessionId": acp_session });
        if let Err(e) = peer.notify("session/cancel", params).await {
            debug!(session = %session_id, "acp cancel failed, dropping connection: {e}");
            engine.conns.close(session_id).await;
        }
    }
    // The agent cancels its outstanding permission requests along with the turn, so a prompt
    // still on screen has nothing left to answer — and answering it would reply into a turn
    // that has already unwound.
    for request_id in engine.clear_session_pending(session_id) {
        engine.emit(
            "",
            "permission.replied",
            json!({ "id": request_id, "requestID": request_id, "sessionID": session_id }),
        );
    }
    engine.set_busy(session_id, false);
}
