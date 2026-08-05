//! The `claude -p` stdout reader: stream-json frames → opencode SSE events.
//!
//! Two granularities arrive on the same stream and are both used:
//! - `stream_event` frames ([`stream_delta`]) carry Anthropic content-block deltas and
//!   drive *live* text/reasoning updates as the model generates them;
//! - `assistant` / `user` / `result` frames mark completed turns; each triggers a
//!   re-parse of the on-disk `<uuid>.jsonl` through the shared transcript parser, which
//!   is authoritative (tool calls, attachments, subagents) and overwrites the streamed
//!   parts in place because the part ids match.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

use super::stream_delta::Partial;
use super::{now_ms, ClaudePEngine};
use crate::claude_engine::{claude_cli, jsonl};

use super::process::message_hash;

/// Read the child's stream-json stdout, driving busy state + message emission.
pub(super) async fn reader(
    engine: Arc<ClaudePEngine>,
    session_id: String,
    stdout: tokio::process::ChildStdout,
    attempted_resume: bool,
) {
    let mut lines = BufReader::new(stdout).lines();
    let mut saw_init = false;
    let mut clean_result = false;
    let mut partial = Partial::default();
    let directory = engine
        .get_session(&session_id)
        .map(|s| s.directory)
        .unwrap_or_default();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "system" => {
                if v.get("subtype").and_then(|s| s.as_str()) == Some("init") {
                    saw_init = true;
                    if let Some(uuid) = v.get("session_id").and_then(|s| s.as_str()) {
                        engine.set_claude_uuid(&session_id, uuid);
                    }
                }
            }
            // Live deltas. Frames tagged with a `parent_tool_use_id` belong to a
            // subagent, not this session — those render through the subagent's own
            // child session on re-parse, so streaming them here would misattribute
            // their text to the parent transcript.
            "stream_event" => {
                if v.get("parent_tool_use_id").map(Value::is_null) != Some(false) {
                    if let Some(event) = v.get("event") {
                        engine.set_busy(&session_id, true);
                        partial.handle(&engine, &session_id, &directory, event);
                    }
                }
            }
            "assistant" => {
                engine.set_busy(&session_id, true);
                reparse_emit(&engine, &session_id).await;
            }
            "user" => reparse_emit(&engine, &session_id).await,
            "result" => {
                reparse_emit(&engine, &session_id).await;
                if let Some(detail) = result_error(&v) {
                    emit_system(&engine, &session_id, "error", &detail);
                }
                clean_result = true;
                engine.set_busy(&session_id, false);
                // Claude's stream-json process may stay alive after a result while
                // no longer accepting another user frame. Treat each result as the
                // turn boundary; the next prompt respawns with --resume, which is
                // reliable for both one-shot and persistent CLI versions.
                break;
            }
            _ => {}
        }
    }
    // EOF: the child exited or was killed. If it died mid-turn (no clean result event),
    // surface that to the frontend — this is the crash/kill case users were hitting.
    if !clean_result && saw_init {
        emit_system(
            &engine,
            &session_id,
            "error",
            "The claude process exited unexpectedly — the turn was interrupted. Send a message to resume.",
        );
    }
    engine.set_busy(&session_id, false);
    engine.procs.forget(&session_id).await;
    // If a resume attempt died before any init event, the stored UUID is likely stale;
    // forget it so the next message starts a fresh conversation instead of looping.
    if attempted_resume && !saw_init {
        engine.forget_claude_uuid(&session_id);
    }
}

/// Turn-level error text from a `result` frame, if it reports one. The transcript may
/// not record these, so they are surfaced as a system bubble instead.
fn result_error(v: &Value) -> Option<String> {
    let is_err = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false)
        || v.get("subtype")
            .and_then(|s| s.as_str())
            .is_some_and(|s| s != "success");
    if !is_err {
        return None;
    }
    Some(
        v.get("result")
            .or_else(|| v.get("error"))
            .and_then(|r| r.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "claude turn ended with an error ({})",
                    v.get("subtype").and_then(|s| s.as_str()).unwrap_or("error")
                )
            }),
    )
}

/// Emit a one-off system bubble (info/warning/error) to the session's frontend. Used for
/// process/turn-level signals that never land in the transcript (crashes, result errors).
pub(super) fn emit_system(engine: &Arc<ClaudePEngine>, session_id: &str, level: &str, text: &str) {
    let Some(sess) = engine.get_session(session_id) else {
        return;
    };
    let variant = match level {
        "error" => "error",
        "warning" | "warn" => "warning",
        _ => "notification",
    };
    let ts = now_ms();
    let mid = format!("msg_sys_{session_id}_{ts}");
    engine.emit(
        &sess.directory,
        "message.updated",
        json!({ "info": {
            "role": "system", "variant": variant, "level": level,
            "id": mid, "sessionID": session_id,
            "time": { "created": ts, "completed": ts },
        }}),
    );
    engine.emit(
        &sess.directory,
        "message.part.updated",
        json!({ "sessionID": session_id, "time": ts, "part": {
            "type": "text", "id": format!("{mid}:0"),
            "messageID": mid, "sessionID": session_id, "text": text,
        }}),
    );
}

/// Re-parse the session's on-disk transcript: register any subagents as child sessions
/// and emit changed messages.
pub(super) async fn reparse_emit(engine: &Arc<ClaudePEngine>, session_id: &str) {
    let Some(sess) = engine.get_session(session_id) else {
        return;
    };
    let Some(uuid) = sess.claude_uuid.clone() else {
        return;
    };
    let Some(path) = claude_cli::locate_jsonl(&uuid) else {
        return;
    };
    reparse_emit_from_path(engine, session_id, &sess.directory, path).await;
}

/// Parse an already-located transcript `path`, register any subagents as child
/// sessions, and emit changed messages for `directory`. Split out of
/// [`reparse_emit`] (which locates the path from the session's claude UUID) so it
/// can be driven directly from a crafted temp transcript in tests.
pub(super) async fn reparse_emit_from_path(
    engine: &Arc<ClaudePEngine>,
    session_id: &str,
    directory: &str,
    path: std::path::PathBuf,
) {
    let sid = session_id.to_string();
    let parsed = tokio::task::spawn_blocking(move || {
        let mut p = jsonl::parse_file(&path, &sid);
        jsonl::enrich_subagents(&mut p);
        jsonl::enrich_background_tasks(&mut p);
        p
    })
    .await;
    let Ok(parsed) = parsed else { return };

    if let Some(title) = &parsed.title {
        engine.set_title(session_id, title, false);
    }
    // Nest any subagents under this session so they appear as child rows.
    for agent_id in &parsed.subagent_ids {
        engine.ensure_subagent_session(session_id, agent_id, "", directory);
    }
    let ts = now_ms();
    for m in &parsed.messages {
        let msg_id = m
            .info
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let h = message_hash(m);
        if !engine.should_emit(session_id, &msg_id, h) {
            debug!(session = %session_id, msg = %msg_id, "unchanged; skipping re-emit");
            continue;
        }
        engine.emit(directory, "message.updated", json!({ "info": m.info }));
        for part in &m.parts {
            engine.emit(
                directory,
                "message.part.updated",
                json!({ "sessionID": session_id, "part": part, "time": ts }),
            );
        }
    }
}
