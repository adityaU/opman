//! Thin wrappers over the `claude` binary's background-agent surface.
//!
//! Validated against claude v2.1.195:
//! - `claude --bg [--resume <uuid>] --permission-mode bypassPermissions [--model m] "<prompt>"`
//!   prints `backgrounded · <shortid>` and starts a detached agent.
//! - `claude agents --json --all [--cwd <dir>]` lists agents as
//!   `[{ id, sessionId, cwd, kind, state|status, name, startedAt }]`.
//! - `claude stop <shortid>` stops an agent (conversation kept).
//! - Transcripts live at `~/.claude/projects/<encoded-cwd>/<sessionId>.jsonl`; we
//!   locate them by globbing on the (unique) UUID to avoid cwd-encoding logic.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};

/// One entry from `claude agents --json`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // some fields are documented but not yet consumed
pub struct AgentInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "sessionId")]
    pub session_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub kind: String,
    /// Background agents report `state` ("working"|"done"|"failed");
    /// interactive ones report `status` ("busy"|"idle").
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub name: String,
}

impl AgentInfo {
    /// Whether this agent is actively working.
    pub fn is_busy(&self) -> bool {
        match self.state.as_deref() {
            Some("working") | Some("running") => return true,
            Some("done") | Some("failed") | Some("completed") | Some("stopped") => return false,
            _ => {}
        }
        matches!(self.status.as_deref(), Some("busy") | Some("working"))
    }
}

/// The configured claude binary (override with `OPMAN_CLAUDE_BIN`).
fn claude_bin() -> String {
    std::env::var("OPMAN_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
}

/// `claude --version` → version string (best-effort).
pub fn version() -> String {
    Command::new(claude_bin())
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "claude".to_string())
}

/// Parse the `backgrounded · <shortid>` line from `claude --bg` output.
fn parse_short_id(output: &str) -> Option<String> {
    for line in output.lines() {
        if line.contains("backgrounded") {
            // e.g. "backgrounded · ae842e84"
            if let Some(tok) = line.split_whitespace().last() {
                let tok = tok.trim();
                if !tok.is_empty() && tok != "backgrounded" {
                    return Some(tok.to_string());
                }
            }
        }
    }
    None
}

/// Options applied to every background turn so permissions/questions route back
/// to opman via the PreToolUse hook.
#[derive(Debug, Clone, Default)]
pub struct TurnOpts {
    pub model: Option<String>,
    pub permission_mode: String,
    /// `--settings` JSON (PreToolUse hook config). Empty = omit.
    pub settings_json: String,
    /// Value for the `OPMAN_ENGINE_URL` env var the hook calls back on.
    pub engine_url: String,
}

fn apply_opts(cmd: &mut Command, opts: &TurnOpts) {
    if !opts.permission_mode.is_empty() {
        cmd.arg("--permission-mode").arg(&opts.permission_mode);
    }
    if !opts.settings_json.is_empty() {
        cmd.arg("--settings").arg(&opts.settings_json);
    }
    if let Some(m) = &opts.model {
        cmd.arg("--model").arg(m);
    }
    if !opts.engine_url.is_empty() {
        cmd.env("OPMAN_ENGINE_URL", &opts.engine_url);
    }
}

/// Start a brand-new background agent. Returns `(short_id, session_uuid)`.
pub fn bg_start(dir: &str, opts: &TurnOpts, prompt: &str) -> Result<(String, String)> {
    let mut cmd = Command::new(claude_bin());
    cmd.arg("--bg");
    apply_opts(&mut cmd, opts);
    cmd.arg(prompt);
    cmd.current_dir(dir);
    run_bg(cmd, dir)
}

/// Continue an existing conversation as a new background turn.
/// Returns the new `(short_id, session_uuid)` (claude mints a fresh UUID per turn).
pub fn bg_resume(
    dir: &str,
    resume_uuid: &str,
    opts: &TurnOpts,
    prompt: &str,
) -> Result<(String, String)> {
    let mut cmd = Command::new(claude_bin());
    cmd.arg("--bg").arg("--resume").arg(resume_uuid);
    apply_opts(&mut cmd, opts);
    cmd.arg(prompt);
    cmd.current_dir(dir);
    run_bg(cmd, dir)
}

fn run_bg(mut cmd: Command, dir: &str) -> Result<(String, String)> {
    let out = cmd
        .output()
        .with_context(|| format!("Failed to spawn `claude --bg` (is `{}` on PATH?)", claude_bin()))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    debug!(%stdout, %stderr, "claude --bg output");

    let short_id = parse_short_id(&stdout)
        .or_else(|| parse_short_id(&stderr))
        .ok_or_else(|| anyhow!("could not parse background short id from claude output: {stdout}{stderr}"))?;

    // Resolve the full session UUID by matching the short id in `agents --json`.
    // The agent registers near-instantly, but retry briefly to avoid a race.
    let mut uuid = String::new();
    for _ in 0..10 {
        if let Ok(agents) = agents_json(Some(dir)) {
            if let Some(a) = agents.iter().find(|a| a.id == short_id) {
                if !a.session_id.is_empty() {
                    uuid = a.session_id.clone();
                    break;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    if uuid.is_empty() {
        warn!(%short_id, "could not resolve full session UUID for background agent");
    }
    Ok((short_id, uuid))
}

/// List background/interactive agents (optionally scoped to a directory).
pub fn agents_json(dir: Option<&str>) -> Result<Vec<AgentInfo>> {
    let mut cmd = Command::new(claude_bin());
    cmd.arg("agents").arg("--json").arg("--all");
    if let Some(d) = dir {
        cmd.arg("--cwd").arg(d);
    }
    let out = cmd.output().context("Failed to run `claude agents --json`")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let agents: Vec<AgentInfo> =
        serde_json::from_str(stdout.trim()).unwrap_or_default();
    Ok(agents)
}

/// Stop a background agent (`claude stop <shortid>`). Conversation is retained.
pub fn stop(short_id: &str) -> Result<()> {
    let _ = Command::new(claude_bin())
        .arg("stop")
        .arg(short_id)
        .output()
        .context("Failed to run `claude stop`")?;
    Ok(())
}

/// Locate a session transcript JSONL by its (unique) UUID.
pub fn locate_jsonl(session_uuid: &str) -> Option<PathBuf> {
    if session_uuid.is_empty() {
        return None;
    }
    let projects = home_projects_dir()?;
    let entries = std::fs::read_dir(&projects).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(format!("{session_uuid}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn home_projects_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("projects"))
}
