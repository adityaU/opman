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

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
    #[serde(default, rename = "startedAt")]
    pub started_at: u64,
}

impl AgentInfo {
    /// Whether this agent's own turn is actively working.
    ///
    /// `state` is authoritative for background agents: "working"/"running" = busy;
    /// "done"/"blocked"/"failed"/etc = this turn is finished. We deliberately do NOT
    /// treat `status == "busy"` as busy on a finished `state` — `status` can linger
    /// "busy" while connections (e.g. attached MCP servers) stay open, which would wedge
    /// the session as permanently busy and block all follow-ups. In-flight *subagents*
    /// (the real reason to stay alive past `state=done`) are tracked separately and
    /// precisely from the transcript (`subagent_pending`), not from this flag.
    pub fn is_busy(&self) -> bool {
        match self.state.as_deref() {
            Some("working") | Some("running") => return true,
            Some("done") | Some("blocked") | Some("failed") | Some("completed")
            | Some("stopped") => return false,
            _ => {}
        }
        // No `state` (e.g. an interactive agent) — fall back to `status`.
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
    /// Agent to run as (`--agent <name>`). None = default agent.
    pub agent: Option<String>,
    pub permission_mode: String,
    /// `--settings` JSON (PreToolUse hook config). Empty = omit.
    pub settings_json: String,
    /// Value for the `OPMAN_ENGINE_URL` env var the hook calls back on.
    pub engine_url: String,
    /// `--mcp-config` JSON attaching opman-managed MCP servers. Empty = omit.
    pub mcp_config: String,
    /// opman session id, exported as `OPENCODE_SESSION_ID` so MCP bridges route
    /// terminal/neovim tools to this session's resources.
    pub session_env_id: String,
}

fn apply_opts(cmd: &mut Command, opts: &TurnOpts) {
    // `--mcp-config` is variadic (`<configs...>`) and would greedily consume the
    // trailing prompt positional, so emit it FIRST — always followed by another
    // flag (`--permission-mode` below is always present) which terminates it.
    if !opts.mcp_config.is_empty() {
        cmd.arg("--mcp-config").arg(&opts.mcp_config);
    }
    let mode = if opts.permission_mode.is_empty() {
        "bypassPermissions"
    } else {
        &opts.permission_mode
    };
    cmd.arg("--permission-mode").arg(mode);
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
    if !opts.session_env_id.is_empty() {
        cmd.env("OPENCODE_SESSION_ID", &opts.session_env_id);
    }
}

/// Detach a background-turn child into its own session (setsid), so the `claude`
/// background service it starts does NOT inherit opman's process group/session and is
/// therefore NOT torn down when opman is signalled or restarted. Without this, an opman
/// restart kills every in-flight background agent (they flip to `state=failed` and their
/// control socket disappears).
fn detach_session(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid() is async-signal-safe; we only call it in the forked child before
    // exec. The child is never a process-group leader (fresh fork), so setsid succeeds.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Start a brand-new background agent. Returns `(short_id, session_uuid)`.
pub fn bg_start(dir: &str, opts: &TurnOpts, prompt: &str) -> Result<(String, String)> {
    let mut cmd = Command::new(claude_bin());
    cmd.arg("--bg");
    apply_opts(&mut cmd, opts);
    cmd.arg(prompt);
    cmd.current_dir(dir);
    detach_session(&mut cmd);
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
    detach_session(&mut cmd);
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

/// One Claude model entry returned by the dynamic model fetch.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    /// Maximum input context tokens.
    pub context_window: u64,
    /// Maximum output tokens.
    pub max_output: u64,
}

/// What claude advertises for a directory in its `system/init` event.
#[derive(Debug, Clone, Default)]
pub struct InitInfo {
    /// Slash commands (built-ins + bundled skills + plugins + custom commands).
    pub commands: Vec<String>,
    /// Real agents available as `--agent <name>` (built-ins + project/user agents).
    pub agents: Vec<String>,
}

/// Introspect what claude exposes for a directory (slash commands + agents).
///
/// The complete lists are only available in claude's `system/init` event. We read
/// just that first line via a stream-json process and kill it immediately — the
/// init event is emitted at startup, before any model request, so this performs
/// **no** model turn (and is not the prompt-running engine, which uses `--bg`).
pub fn introspect(dir: &str) -> InitInfo {
    let mut child = match Command::new(claude_bin())
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            "haiku",
            ".",
        ])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to introspect claude init event: {e}");
            return InitInfo::default();
        }
    };

    let mut info = InitInfo::default();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let is_init = v.get("type").and_then(|t| t.as_str()) == Some("system")
                && v.get("subtype").and_then(|s| s.as_str()) == Some("init");
            if is_init {
                let str_array = |key: &str| -> Vec<String> {
                    v.get(key)
                        .and_then(|s| s.as_array())
                        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                        .unwrap_or_default()
                };
                info.commands = str_array("slash_commands");
                info.agents = str_array("agents");
                break; // got what we need — stop before any model work
            }
        }
    }
    // Abort the introspection process (no turn is completed).
    let _ = child.kill();
    let _ = child.wait();
    info
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

/// Locate a subagent transcript by its `agentId`.
///
/// Subagents write to `~/.claude/projects/<encoded-cwd>/<parent-uuid>/subagents/
/// agent-<agentId>.jsonl`. The parent UUID changes per `--resume` turn, so we glob
/// across project dirs and their per-turn UUID subdirectories.
pub fn locate_subagent_jsonl(agent_id: &str) -> Option<PathBuf> {
    if agent_id.is_empty() {
        return None;
    }
    let projects = home_projects_dir()?;
    let fname = format!("agent-{agent_id}.jsonl");
    for proj in std::fs::read_dir(&projects).ok()?.flatten() {
        let pdir = proj.path();
        if !pdir.is_dir() {
            continue;
        }
        let Ok(turns) = std::fs::read_dir(&pdir) else {
            continue;
        };
        for turn in turns.flatten() {
            let candidate = turn.path().join("subagents").join(&fname);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Derive a human-readable display name from a model ID.
/// e.g. "claude-opus-4-8" → "Claude Opus 4.8"
fn model_display_name(id: &str) -> String {
    // Strip "claude-" prefix
    let bare = id.strip_prefix("claude-").unwrap_or(id);
    // Strip [1m] or similar bracket suffixes
    let (bare, variant) = if let Some(idx) = bare.find('[') {
        let inner = bare[idx + 1..].trim_end_matches(']');
        (bare[..idx].trim_end_matches('-'), format!(" ({inner})"))
    } else {
        (bare, String::new())
    };
    // Remove trailing date segment: "-YYYYMMDD" (8+ digits)
    let bare = match bare.rfind('-') {
        Some(pos) if bare[pos + 1..].len() >= 8 && bare[pos + 1..].chars().all(|c| c.is_ascii_digit()) => {
            &bare[..pos]
        }
        _ => bare,
    };
    // Split into segments: first alphabetic segment(s) = tier, rest = version numbers
    let segs: Vec<&str> = bare.split('-').collect();
    let tier_end = segs.iter().position(|s| s.chars().all(|c| c.is_ascii_digit())).unwrap_or(segs.len());
    let tier: String = segs[..tier_end]
        .iter()
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let version = segs[tier_end..].join(".");
    if version.is_empty() {
        format!("Claude {tier}{variant}")
    } else {
        format!("Claude {tier} {version}{variant}")
    }
}

/// Conservative context/output limits keyed by model name patterns.
fn model_limits(id: &str) -> (u64, u64) {
    let n = id.to_lowercase();
    if n.contains("[1m]") || n.contains("1m]") {
        return (1_000_000, 128_000);
    }
    if n.contains("opus") {
        (1_000_000, 128_000)
    } else if n.contains("sonnet-5") || n.contains("fable") {
        (1_000_000, 128_000)
    } else if n.contains("sonnet") {
        (200_000, 64_000)
    } else if n.contains("haiku") {
        (200_000, 32_000)
    } else {
        (200_000, 64_000)
    }
}

/// Fetch available Claude models by running a lightweight `claude -p` turn.
///
/// Runs `claude -p --model haiku --output-format json` with a structured prompt,
/// parses the result field for a JSON array of model IDs, and derives display
/// names and context-window limits from known patterns.
///
/// Returns `None` if the CLI call fails or the output cannot be parsed — the caller
/// should fall back to a hardcoded default list.
pub fn fetch_models_via_cli() -> Option<Vec<ModelInfo>> {
    let out = Command::new(claude_bin())
        .args([
            "-p",
            "--model",
            "haiku",
            "--output-format",
            "json",
            "Output ONLY a raw JSON array of currently available Claude model IDs. \
             No explanation, no markdown, no code block — just the array. \
             Example: [\"claude-haiku-4-5-20251001\",\"claude-sonnet-5\"]",
        ])
        .output()
        .ok()?;

    if !out.status.success() {
        warn!("claude -p model fetch exited non-zero");
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let raw = v.get("result")?.as_str()?;

    // Strip optional markdown code-fence wrapping
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let ids: Vec<String> = serde_json::from_str(stripped).ok()?;
    if ids.is_empty() {
        return None;
    }

    Some(
        ids.into_iter()
            .map(|id| {
                let display_name = model_display_name(&id);
                let (context_window, max_output) = model_limits(&id);
                ModelInfo { id, display_name, context_window, max_output }
            })
            .collect(),
    )
}

fn home_projects_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("projects"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(state: Option<&str>, status: Option<&str>) -> AgentInfo {
        AgentInfo {
            id: "x".into(),
            session_id: "u".into(),
            cwd: "/d".into(),
            kind: "background".into(),
            state: state.map(String::from),
            status: status.map(String::from),
            name: String::new(),
            started_at: 0,
        }
    }

    // `state` is authoritative: a finished turn is idle regardless of a lingering
    // "busy" status (which can be held open by attached MCP connections). Subagent
    // liveness past state=done is tracked separately via the transcript.
    #[test]
    fn finished_state_is_idle_even_if_status_busy() {
        assert!(!agent(Some("done"), Some("busy")).is_busy());
        assert!(!agent(Some("completed"), Some("busy")).is_busy());
        assert!(!agent(Some("blocked"), Some("idle")).is_busy());
        assert!(!agent(Some("failed"), Some("busy")).is_busy());
    }

    #[test]
    fn working_state_is_busy() {
        assert!(agent(Some("working"), Some("busy")).is_busy());
        assert!(agent(Some("running"), None).is_busy());
    }

    #[test]
    fn status_is_only_a_fallback_when_state_absent() {
        assert!(agent(None, Some("busy")).is_busy());
        assert!(!agent(None, Some("idle")).is_busy());
        assert!(!agent(None, None).is_busy());
    }
}
