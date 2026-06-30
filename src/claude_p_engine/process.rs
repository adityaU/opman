//! The persistent `claude -p` process driver.
//!
//! One long-lived `claude -p --input-format stream-json` child per opman session. The
//! child is a read-eval loop over newline-delimited user messages on stdin, so:
//! - **push / steering**: a follow-up is written straight to the running child's stdin
//!   (no queue, no wait — delivered even mid-turn);
//! - **hard abort**: the child is killed outright (`start_kill`), ending the turn now;
//! - **continuity**: a respawn (after abort/restart) passes `--resume <uuid>` so the
//!   conversation continues with full history.
//!
//! Message rendering reuses the shared transcript parser: on each stream event we
//! re-parse the on-disk `<uuid>.jsonl` (which `claude -p` writes in the same format the
//! background engine uses) and emit only the messages whose content changed. Subagents
//! referenced by the transcript are registered as nested child sessions.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::{now_ms, ClaudePEngine};
use crate::claude_engine::{claude_cli, jsonl};

/// Resolved arguments for a session's `claude -p` process.
pub(super) struct TurnOpts {
    pub model: Option<String>,
    pub agent: Option<String>,
    pub permission_mode: String,
    pub settings_json: String,
    pub engine_url: String,
    pub mcp_config: String,
    pub session_env_id: String,
    /// claude UUID to `--resume` (continue a prior conversation), if any.
    pub resume_uuid: Option<String>,
}

/// A live `claude -p` child: its stdin (to push messages) and the handle (to kill it).
struct Proc {
    stdin: ChildStdin,
    child: Child,
}

/// Live processes keyed by opman session id.
#[derive(Default)]
pub struct ProcMap(Mutex<HashMap<String, Proc>>);

fn claude_bin() -> String {
    std::env::var("OPMAN_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
}

/// Stable hash of a rendered message's content (info + parts).
fn message_hash(msg: &jsonl::MsgOut) -> u64 {
    let mut h = DefaultHasher::new();
    msg.info.to_string().hash(&mut h);
    for p in &msg.parts {
        p.to_string().hash(&mut h);
    }
    h.finish()
}

/// Send a user message to the session's process, spawning it on first use. The message
/// is pushed to the running child's stdin — true steering, never queued.
pub async fn send(engine: Arc<ClaudePEngine>, session_id: String, text: String) {
    let Some(sess) = engine.get_session(&session_id) else { return };
    let dir = sess.directory;

    let mut procs = engine.procs.0.lock().await;
    if !procs.contains_key(&session_id) {
        match spawn(&engine, &session_id, &dir).await {
            Ok(proc) => {
                procs.insert(session_id.clone(), proc);
            }
            Err(e) => {
                warn!(session = %session_id, "claude -p spawn failed: {e}");
                engine.set_busy(&session_id, false);
                return;
            }
        }
    }

    let Some(proc) = procs.get_mut(&session_id) else { return };
    let line = json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": text }] }
    })
    .to_string();
    let ok = proc.stdin.write_all(line.as_bytes()).await.is_ok()
        && proc.stdin.write_all(b"\n").await.is_ok()
        && proc.stdin.flush().await.is_ok();
    if ok {
        engine.set_busy(&session_id, true);
    } else {
        debug!(session = %session_id, "claude -p stdin closed; dropping process");
        procs.remove(&session_id);
        engine.set_busy(&session_id, false);
    }
}

/// Hard-abort: kill the session's process immediately. The conversation transcript is
/// retained, so the next message respawns with `--resume` and continues with full
/// history. Idempotent.
pub async fn abort(engine: Arc<ClaudePEngine>, session_id: &str) {
    let removed = {
        let mut procs = engine.procs.0.lock().await;
        procs.remove(session_id)
    };
    if let Some(mut p) = removed {
        let _ = p.child.start_kill();
        let _ = p.child.wait().await;
    }
    engine.set_busy(session_id, false);
}

async fn spawn(engine: &Arc<ClaudePEngine>, session_id: &str, dir: &str) -> Result<Proc> {
    let opts = engine.turn_opts(session_id, dir);
    let mut cmd = Command::new(claude_bin());
    cmd.arg("-p")
        .args(["--input-format", "stream-json", "--output-format", "stream-json", "--verbose"]);
    // `--mcp-config` is variadic, so emit it before the always-present `--permission-mode`.
    if !opts.mcp_config.is_empty() {
        cmd.arg("--mcp-config").arg(&opts.mcp_config);
    }
    let mode = if opts.permission_mode.is_empty() {
        "bypassPermissions"
    } else {
        &opts.permission_mode
    };
    cmd.arg("--permission-mode").arg(mode);
    // Continue a prior conversation (post-abort / post-restart) with full history.
    if let Some(uuid) = &opts.resume_uuid {
        cmd.arg("--resume").arg(uuid);
    }
    if !opts.settings_json.is_empty() {
        cmd.arg("--settings").arg(&opts.settings_json);
    }
    if let Some(m) = &opts.model {
        cmd.arg("--model").arg(m);
    }
    if let Some(a) = &opts.agent {
        if !a.is_empty() {
            cmd.arg("--agent").arg(a);
        }
    }
    if !opts.engine_url.is_empty() {
        cmd.env("OPMAN_ENGINE_URL", &opts.engine_url);
    }
    cmd.env("OPENCODE_SESSION_ID", &opts.session_env_id);
    cmd.current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn `{} -p` (is it on PATH?)", claude_bin()))?;
    let stdin = child.stdin.take().context("claude -p: no stdin")?;
    let stdout = child.stdout.take().context("claude -p: no stdout")?;

    tokio::spawn(reader(engine.clone(), session_id.to_string(), stdout, opts.resume_uuid.is_some()));
    Ok(Proc { stdin, child })
}

/// Read the child's stream-json stdout, driving busy state + message emission.
async fn reader(
    engine: Arc<ClaudePEngine>,
    session_id: String,
    stdout: tokio::process::ChildStdout,
    attempted_resume: bool,
) {
    let mut lines = BufReader::new(stdout).lines();
    let mut saw_init = false;
    let mut clean_result = false;
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "system" => {
                if v.get("subtype").and_then(|s| s.as_str()) == Some("init") {
                    saw_init = true;
                    if let Some(uuid) = v.get("session_id").and_then(|s| s.as_str()) {
                        engine.set_claude_uuid(&session_id, uuid);
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
                // Surface a turn-level error (the transcript may not record it).
                let is_err = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false)
                    || v.get("subtype").and_then(|s| s.as_str()).is_some_and(|s| s != "success");
                if is_err {
                    let detail = v
                        .get("result")
                        .or_else(|| v.get("error"))
                        .and_then(|r| r.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            format!(
                                "claude turn ended with an error ({})",
                                v.get("subtype").and_then(|s| s.as_str()).unwrap_or("error")
                            )
                        });
                    emit_system(&engine, &session_id, "error", &detail);
                }
                clean_result = true;
                engine.set_busy(&session_id, false);
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
    engine.procs.0.lock().await.remove(&session_id);
    // If a resume attempt died before any init event, the stored UUID is likely stale;
    // forget it so the next message starts a fresh conversation instead of looping.
    if attempted_resume && !saw_init {
        engine.forget_claude_uuid(&session_id);
    }
}

/// Emit a one-off system bubble (info/warning/error) to the session's frontend. Used for
/// process/turn-level signals that never land in the transcript (crashes, result errors).
fn emit_system(engine: &Arc<ClaudePEngine>, session_id: &str, level: &str, text: &str) {
    let Some(sess) = engine.get_session(session_id) else { return };
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
async fn reparse_emit(engine: &Arc<ClaudePEngine>, session_id: &str) {
    let Some(sess) = engine.get_session(session_id) else { return };
    let Some(uuid) = sess.claude_uuid.clone() else { return };
    let Some(path) = claude_cli::locate_jsonl(&uuid) else { return };

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
        engine.ensure_subagent_session(session_id, agent_id, "", &sess.directory);
    }
    let ts = now_ms();
    for m in &parsed.messages {
        let msg_id = m.info.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let h = message_hash(m);
        if !engine.should_emit(session_id, &msg_id, h) {
            continue;
        }
        engine.emit(&sess.directory, "message.updated", json!({ "info": m.info }));
        for part in &m.parts {
            engine.emit(
                &sess.directory,
                "message.part.updated",
                json!({ "sessionID": session_id, "part": part, "time": ts }),
            );
        }
    }
}
