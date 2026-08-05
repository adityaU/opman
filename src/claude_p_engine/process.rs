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
//! Stdout handling lives in [`super::stream`]: content-block deltas render live, and
//! each completed block re-parses the on-disk `<uuid>.jsonl` (which `claude -p` writes
//! in the same format the background engine uses) as the authoritative rendering.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::stream::reader;
use super::ClaudePEngine;
use crate::claude_engine::jsonl;

#[cfg(test)]
pub(super) use super::stream::{emit_system, reparse_emit, reparse_emit_from_path};

/// Resolved arguments for a session's `claude -p` process.
pub(super) struct TurnOpts {
    pub model: Option<String>,
    pub agent: Option<String>,
    pub effort: Option<String>,
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

impl ProcMap {
    /// Drop a session's child handle, if one is still registered. Used by the stdout
    /// reader once the stream ends, so the next prompt spawns a fresh `--resume` child.
    pub(super) async fn forget(&self, session_id: &str) {
        self.0.lock().await.remove(session_id);
    }
}

fn claude_bin() -> String {
    std::env::var("OPMAN_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
}

/// Stable hash of a rendered message's content (info + parts).
pub(super) fn message_hash(msg: &jsonl::MsgOut) -> u64 {
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
    let Some(session) = engine.get_session(&session_id) else {
        return;
    };
    if session.busy {
        schedule_idle_retry(engine, session_id, text);
        return;
    }
    send_ready(engine, session_id, text, true).await;
}

async fn send_ready(
    engine: Arc<ClaudePEngine>,
    session_id: String,
    text: String,
    allow_retry: bool,
) {
    let Some(sess) = engine.get_session(&session_id) else {
        return;
    };
    let dir = sess.directory;
    let resume_turn = sess.claude_uuid.is_some();

    let mut procs = engine.procs.0.lock().await;
    if resume_turn {
        // A completed stream may remain in the map briefly after its result.
        // Never write a follow-up into that stale child; resume the conversation
        // through a fresh process instead.
        procs.remove(&session_id);
    }
    if !procs.contains_key(&session_id) {
        match spawn(&engine, &session_id, &dir).await {
            Ok(proc) => {
                procs.insert(session_id.clone(), proc);
            }
            Err(e) => {
                warn!(session = %session_id, "claude -p spawn failed: {e}");
                if allow_retry {
                    schedule_resume_retry(&engine, &session_id, text);
                } else {
                    engine.set_busy(&session_id, false);
                }
                return;
            }
        }
    }

    let Some(proc) = procs.get_mut(&session_id) else {
        return;
    };
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
        debug!(session = %session_id, "claude -p stdin closed; retrying with resume");
        procs.remove(&session_id);
        if allow_retry {
            schedule_resume_retry(&engine, &session_id, text);
        } else {
            engine.set_busy(&session_id, false);
        }
    }
}

fn schedule_idle_retry(engine: Arc<ClaudePEngine>, session_id: String, text: String) {
    tokio::spawn(async move {
        for _ in 0..240 {
            let idle = engine
                .get_session(&session_id)
                .map(|session| !session.busy)
                .unwrap_or(true);
            if idle {
                send(engine, session_id, text).await;
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    });
}

fn schedule_resume_retry(engine: &Arc<ClaudePEngine>, session_id: &str, text: String) {
    let can_resume = engine
        .get_session(session_id)
        .and_then(|session| session.claude_uuid)
        .is_some();
    if !can_resume {
        engine.set_busy(session_id, false);
        return;
    }

    let engine = engine.clone();
    let session_id = session_id.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        send_ready(engine, session_id, text, false).await;
    });
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
    cmd.arg("-p").args([
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        // Emit Anthropic content-block deltas, so text and reasoning render as the
        // model generates them instead of landing whole when a block completes.
        "--include-partial-messages",
    ]);
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
    if let Some(effort) = &opts.effort {
        if !effort.is_empty() {
            cmd.arg("--effort").arg(effort);
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

    tokio::spawn(reader(
        engine.clone(),
        session_id.to_string(),
        stdout,
        opts.resume_uuid.is_some(),
    ));
    Ok(Proc { stdin, child })
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod process_tests;

#[cfg(test)]
#[path = "process_driver_tests.rs"]
mod process_driver_tests;

#[cfg(test)]
#[path = "process_spawn_args_tests.rs"]
mod process_spawn_args_tests;

#[cfg(test)]
#[path = "process_reader_edge_tests.rs"]
mod process_reader_edge_tests;
