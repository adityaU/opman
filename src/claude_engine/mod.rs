//! Embedded opencode-compatible HTTP/SSE adapter, backed by the `claude` CLI.
//!
//! When opman runs with `--claude` (backend = ClaudeCode), this in-process axum
//! server stands in for `opencode serve`. opman's `api/`, `sse/`, `web/`, Slack, and
//! stats layers keep speaking the opencode REST + SSE contract to `base_url`,
//! unaware that the backend is now `claude` background agents.
//!
//! See `registry.rs` (session id mapping), `claude_cli.rs` (process wrappers),
//! `jsonl.rs` (transcript → messages), `events.rs` (transcript → SSE events),
//! `tailer.rs` (live event streaming + busy/idle polling), `routes.rs` (REST).

pub(crate) mod claude_cli;
mod events;
pub(crate) mod jsonl;
pub(crate) mod models;
mod reaper;
mod registry;
mod routes;
mod tailer;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use tokio::sync::{broadcast, oneshot};
use tracing::info;

use crate::server::ServerHandle;
use registry::{Registry, SessionEntry};

/// How opman answered a pending permission/question request from the hook.
#[derive(Debug)]
pub enum PendingReply {
    /// Permission reply: "once" | "always" | "reject".
    Permission(String),
    /// Question answers: one list of selected labels per question.
    Question(Vec<Vec<String>>),
    /// Question/permission was rejected/dismissed.
    Reject,
}

/// A translated opencode event, tagged with the project directory it belongs to so
/// the per-project SSE stream can filter to the right subscriber.
#[derive(Clone, Debug)]
pub struct EngineEvent {
    pub directory: String,
    /// Pre-serialized `{ "type": …, "properties": … }` JSON.
    pub data: String,
}

/// The embedded Claude engine: session registry + event bus.
pub struct ClaudeEngine {
    reg: Mutex<Registry>,
    persist: Option<PathBuf>,
    events: broadcast::Sender<EngineEvent>,
    tailers: Mutex<HashSet<String>>,
    /// In-flight permission/question requests awaiting an opman reply.
    pending: Mutex<HashMap<String, oneshot::Sender<PendingReply>>>,
    /// Engine HTTP base URL (set once bound); passed to hook subprocesses.
    url: Mutex<String>,
    /// Default permission mode when a session has no override.
    default_mode: String,
    /// Path to the opman executable (used as the PreToolUse hook command).
    exe: PathBuf,
    /// Cache of discovered claude slash commands, keyed by project directory.
    command_cache: Mutex<HashMap<String, claude_cli::InitInfo>>,
    /// Every MCP server to attach to a turn, already resolved from built-ins plus
    /// `mcp.json`. Shared with the other engines rather than re-derived here.
    mcp: crate::mcp_registry::SharedRegistry,
    /// Follow-up prompts queued while a session's agent is still running. Flushed (as
    /// a single `--resume` turn) once the session goes fully idle — we never resume a
    /// live agent, which would spawn a competing process and orphan its subagents.
    pending_prompts: Mutex<HashMap<String, Vec<String>>>,
    /// Sessions with a turn currently being spawned (between the decision to run and the
    /// agent registering in `claude agents`). Treated as busy so the status poller can't
    /// race it to "idle" and trigger a duplicate/competing turn.
    dispatching: Mutex<HashSet<String>>,
    /// Sessions the user just aborted, → the abort timestamp (ms). `claude stop` is
    /// graceful, so the agent can linger `state=working` for a poll or two; while a
    /// session is "settling" the poller forces it idle instead of bouncing it back to
    /// busy. Cleared once the agent actually stops, on a new turn, or after a safety cap.
    aborting: Mutex<HashMap<String, u64>>,
    /// Cached dynamic model list from `claude -p` with the fetch timestamp (ms).
    /// Refreshed every hour by the `/provider` route.
    model_cache: Mutex<Option<(Vec<claude_cli::ModelInfo>, u64)>>,
}

static ENGINE: OnceLock<Arc<ClaudeEngine>> = OnceLock::new();

/// Global accessor (used by the PTY layer to resolve a session's `claude` short id).
pub fn engine() -> Option<Arc<ClaudeEngine>> {
    ENGINE.get().cloned()
}

/// The current `claude` background short id for a session (for `claude attach`).
pub fn short_id_for_session(session_id: &str) -> Option<String> {
    engine()
        .and_then(|e| e.get_session(session_id))
        .and_then(|s| s.short_id)
}

/// Upper bound on how long a session stays "settling" after an abort before the poller
/// stops force-idling it. `claude stop` is graceful: the agent's `state` can stay
/// `working` briefly, and a just-killed subagent's transcript stays "fresh" (mtime) for a
/// while. We keep forcing idle past both so an abort doesn't visibly bounce back to busy;
/// a new turn clears it sooner. Sized to outlast the subagent staleness window.
const ABORT_SETTLE_MS: u64 = 240_000;

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn rand_id(prefix: &str) -> String {
    let n: u128 = rand::random();
    format!("{prefix}_{n:032x}")
}

/// Engine-wide default model (`OPMAN_CLAUDE_MODEL`), if set.
fn default_model() -> Option<String> {
    std::env::var("OPMAN_CLAUDE_MODEL")
        .ok()
        .filter(|s| !s.is_empty())
}

impl ClaudeEngine {
    fn new(persist: Option<PathBuf>, mcp: crate::mcp_registry::SharedRegistry) -> Self {
        let reg = match &persist {
            Some(p) => Registry::load(p),
            None => Registry::default(),
        };
        let (events, _) = broadcast::channel(2048);
        // Default to bypassPermissions (no per-tool prompts) — `AskUserQuestion`
        // still surfaces to opman regardless of mode, and the user can switch to a
        // prompting mode at runtime via `/permission-mode <mode>`.
        let default_mode = std::env::var("OPMAN_CLAUDE_PERMISSION_MODE")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "bypassPermissions".to_string());
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("opman"));
        Self {
            reg: Mutex::new(reg),
            persist,
            events,
            tailers: Mutex::new(HashSet::new()),
            pending: Mutex::new(HashMap::new()),
            url: Mutex::new(String::new()),
            default_mode,
            exe,
            command_cache: Mutex::new(HashMap::new()),
            mcp,
            pending_prompts: Mutex::new(HashMap::new()),
            dispatching: Mutex::new(HashSet::new()),
            aborting: Mutex::new(HashMap::new()),
            model_cache: Mutex::new(None),
        }
    }

    /// Build the `--mcp-config` payload for a turn, or `None` when nothing is offered so
    /// the flag can be omitted rather than passed an empty object.
    ///
    /// Rebuilt per turn rather than cached, because a server's availability can change
    /// mid-run: the kanban bridge appears as soon as the web server writes its loopback
    /// descriptor.
    pub fn mcp_config_json(&self, dir: &str, session_id: &str) -> Option<String> {
        let registry = self.mcp.current();
        crate::mcp_registry::render::claude_mcp_config(
            registry.for_runner(&crate::runner::RunnerKind::ClaudeCode),
            registry.bind(dir, Some(session_id)),
        )
    }

    fn set_url(&self, url: &str) {
        if let Ok(mut u) = self.url.lock() {
            *u = url.to_string();
        }
    }

    pub fn url(&self) -> String {
        self.url.lock().map(|u| u.clone()).unwrap_or_default()
    }

    pub fn exe(&self) -> PathBuf {
        self.exe.clone()
    }

    /// Effective permission mode for a session (override → engine default).
    pub fn effective_mode(&self, session_id: &str) -> String {
        self.get_session(session_id)
            .and_then(|s| s.permission_mode)
            .unwrap_or_else(|| self.default_mode.clone())
    }

    /// Set a session's permission mode at runtime; returns the new mode.
    pub fn set_permission_mode(&self, session_id: &str, mode: &str) {
        let dir = {
            let mut g = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(e) = g.sessions.get_mut(session_id) else {
                return;
            };
            e.permission_mode = Some(mode.to_string());
            e.directory.clone()
        };
        self.save();
        // Surface a toast so TUI/web confirm the change.
        self.emit(
            &dir,
            "tui.toast.show",
            serde_json::json!({ "message": format!("Claude permission mode: {mode}"), "variant": "info" }),
        );
    }

    /// Set the model (claude `--model` value) used for a session's turns.
    pub fn set_model(&self, session_id: &str, model: &str) {
        if model.is_empty() {
            return;
        }
        let mut changed = false;
        if let Ok(mut g) = self.reg.lock() {
            if let Some(e) = g.sessions.get_mut(session_id) {
                if e.model.as_deref() != Some(model) {
                    e.model = Some(model.to_string());
                    changed = true;
                }
            }
        }
        if changed {
            self.save();
        }
    }

    /// Set the reasoning effort (`--effort`) used for a session's turns.
    pub fn set_effort(&self, session_id: &str, effort: &str) {
        let effort = effort.trim();
        if effort.is_empty() {
            return;
        }
        let mut changed = false;
        if let Ok(mut g) = self.reg.lock() {
            if let Some(e) = g.sessions.get_mut(session_id) {
                if e.effort.as_deref() != Some(effort) {
                    e.effort = Some(effort.to_string());
                    changed = true;
                }
            }
        }
        if changed {
            self.save();
        }
    }

    /// Resolve a requested agent name to a real claude agent for this session's project.
    ///
    /// Kanban lanes and the opencode picker speak opencode agent names (`build`, `plan`,
    /// `code-reviewer`); claude has a different set (`claude`, `Plan`, `Explore`, …, plus
    /// project `.claude/agents/*`). We match the request (case-insensitively) against the
    /// real agent list from claude's init event, translate well-known opencode aliases,
    /// and otherwise fall back to the default agent (empty → no `--agent`, which is the
    /// normal coding agent). This avoids the "no agent named 'build'" warning and a turn
    /// silently running under the wrong/template agent.
    fn resolve_agent(&self, session_id: &str, agent: &str) -> String {
        let agent = agent.trim();
        if agent.is_empty() {
            return String::new();
        }
        let dir = self
            .get_session(session_id)
            .map(|s| s.directory)
            .unwrap_or_default();
        let known = self.cached_init(&dir).map(|i| i.agents).unwrap_or_default();
        let find = |name: &str| known.iter().find(|a| a.eq_ignore_ascii_case(name)).cloned();

        // Already a real claude agent (exact or case-insensitive) → use its real casing.
        if let Some(real) = find(agent) {
            return real;
        }
        // Translate well-known opencode aliases.
        match agent.to_ascii_lowercase().as_str() {
            // opencode's planning agent → claude's `Plan`.
            "plan" => find("Plan").unwrap_or_else(|| {
                if known.is_empty() {
                    "Plan".to_string()
                } else {
                    String::new()
                }
            }),
            // opencode's default coding agent and the code reviewer have no claude
            // equivalent — run under claude's default agent.
            "build" | "code-reviewer" | "reviewer" => String::new(),
            // Unknown name: if we couldn't introspect yet, pass it through (might be a
            // real project agent); otherwise it isn't real → default agent.
            _ => {
                if known.is_empty() {
                    agent.to_string()
                } else {
                    String::new()
                }
            }
        }
    }

    /// Set the agent (`--agent`) for a session, translating opencode agent names to the
    /// project's real claude agents (see [`resolve_agent`]). An unresolved name clears the
    /// override so the turn runs under claude's default agent.
    pub fn set_agent(&self, session_id: &str, agent: &str) {
        let resolved = self.resolve_agent(session_id, agent);
        let mut changed = false;
        if let Ok(mut g) = self.reg.lock() {
            if let Some(e) = g.sessions.get_mut(session_id) {
                let new = (!resolved.is_empty()).then(|| resolved.clone());
                if e.agent != new {
                    e.agent = new;
                    changed = true;
                }
            }
        }
        if changed {
            self.save();
        }
    }

    /// Cached init introspection (commands + agents) for a directory, if discovered.
    pub fn cached_init(&self, dir: &str) -> Option<claude_cli::InitInfo> {
        self.command_cache.lock().ok()?.get(dir).cloned()
    }

    /// Store discovered init introspection for a directory.
    pub fn set_cached_init(&self, dir: &str, info: claude_cli::InitInfo) {
        if let Ok(mut c) = self.command_cache.lock() {
            c.insert(dir.to_string(), info);
        }
    }

    /// Return the cached model list if one has been stored (no TTL — updated only at startup).
    pub fn cached_models_any(&self) -> Option<Vec<claude_cli::ModelInfo>> {
        let guard = self.model_cache.lock().ok()?;
        Some(guard.as_ref()?.0.clone())
    }

    /// Store the model list fetched at startup.
    pub fn set_cached_models(&self, models: Vec<claude_cli::ModelInfo>) {
        if let Ok(mut g) = self.model_cache.lock() {
            *g = Some((models, now_ms()));
        }
    }

    fn add_allowed_tool(&self, session_id: &str, tool: &str) {
        if let Ok(mut g) = self.reg.lock() {
            if let Some(e) = g.sessions.get_mut(session_id) {
                if !e.allowed_tools.iter().any(|t| t == tool) {
                    e.allowed_tools.push(tool.to_string());
                }
            }
        }
        self.save();
    }

    fn is_always_allowed(&self, session_id: &str, tool: &str) -> bool {
        self.get_session(session_id)
            .map(|s| s.allowed_tools.iter().any(|t| t == tool))
            .unwrap_or(false)
    }

    /// Register a pending hook request; returns the receiver to await opman's reply.
    pub fn register_pending(&self, id: &str) -> oneshot::Receiver<PendingReply> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut p) = self.pending.lock() {
            p.insert(id.to_string(), tx);
        }
        rx
    }

    /// Resolve a pending request (called by the reply endpoints). Returns false if
    /// the id was unknown (already resolved or expired).
    pub fn resolve_pending(&self, id: &str, reply: PendingReply) -> bool {
        let tx = self.pending.lock().ok().and_then(|mut p| p.remove(id));
        match tx {
            Some(tx) => tx.send(reply).is_ok(),
            None => false,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    fn save(&self) {
        if let Some(p) = &self.persist {
            if let Ok(reg) = self.reg.lock() {
                reg.save(p);
            }
        }
    }

    /// Emit an opencode-shaped event for `{type, properties}` scoped to `directory`.
    pub fn emit(&self, directory: &str, event_type: &str, properties: serde_json::Value) {
        let payload = serde_json::json!({ "type": event_type, "properties": properties });
        let _ = self.events.send(EngineEvent {
            directory: directory.to_string(),
            data: payload.to_string(),
        });
    }

    /// Emit a one-off system bubble (info/warning/error) to a session's frontend, for
    /// process/turn-level signals that never reach the transcript (spawn failures, crashes).
    pub fn emit_system(&self, session_id: &str, level: &str, text: &str) {
        let Some(dir) = self.get_session(session_id).map(|s| s.directory) else {
            return;
        };
        let variant = match level {
            "error" => "error",
            "warning" | "warn" => "warning",
            _ => "notification",
        };
        let ts = now_ms();
        let mid = format!("msg_sys_{session_id}_{ts}");
        self.emit(
            &dir,
            "message.updated",
            serde_json::json!({ "info": {
                "role": "system", "variant": variant, "level": level,
                "id": mid, "sessionID": session_id,
                "time": { "created": ts, "completed": ts },
            }}),
        );
        self.emit(
            &dir,
            "message.part.updated",
            serde_json::json!({ "sessionID": session_id, "time": ts, "part": {
                "type": "text", "id": format!("{mid}:0"),
                "messageID": mid, "sessionID": session_id, "text": text,
            }}),
        );
    }

    pub fn get_session(&self, id: &str) -> Option<SessionEntry> {
        self.reg.lock().ok()?.sessions.get(id).cloned()
    }

    pub fn list_for_dir(&self, dir: &str) -> Vec<SessionEntry> {
        self.reg
            .lock()
            .map(|r| r.for_directory(dir))
            .unwrap_or_default()
    }

    /// Import existing `claude` sessions for a directory into the registry so they
    /// appear in opman's sidebar (e.g. prior conversations from earlier runs or
    /// from using `claude` directly). Idempotent — already-mapped UUIDs are skipped.
    pub fn import_agents(&self, dir: &str, mut agents: Vec<claude_cli::AgentInfo>) {
        let now = now_ms();
        let mut changed = false;
        // Known UUIDs (latest + full lineage, so `--bg --resume` turns aren't dup'd),
        // tombstoned UUIDs (deleted — never resurrect), and titles already shown for
        // this dir (so same-titled resume chains collapse to one entry).
        let (known, deleted, mut seen_titles) = self
            .reg
            .lock()
            .map(|g| {
                let mut known = std::collections::HashSet::new();
                let mut titles = std::collections::HashSet::new();
                for sess in g.sessions.values() {
                    if let Some(c) = &sess.claude_session_id {
                        known.insert(c.clone());
                    }
                    for u in &sess.lineage {
                        known.insert(u.clone());
                    }
                    if sess.directory == dir {
                        titles.insert(sess.title.to_lowercase());
                    }
                }
                (known, g.deleted.clone(), titles)
            })
            .unwrap_or_default();

        // Newest first, so the surviving entry for a duplicate title is the latest run.
        agents.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        if let Ok(mut g) = self.reg.lock() {
            for a in agents {
                if a.session_id.is_empty() || a.cwd != dir {
                    continue;
                }
                if known.contains(&a.session_id) || deleted.contains(&a.session_id) {
                    continue; // already represented, or deleted by the user
                }
                let Some(path) = claude_cli::locate_jsonl(&a.session_id) else {
                    continue; // no transcript yet — nothing to show
                };
                // Only import sessions claude gave a generated title (real
                // conversations). Trivial/command/aborted runs lack an ai-title and
                // would otherwise flood the sidebar with duplicate prompt-name entries.
                let Some(title) = jsonl::read_ai_title(&path).filter(|t| !t.is_empty()) else {
                    continue;
                };
                // Collapse same-titled sessions (resume chains from earlier runs) to
                // one entry — the newest (we sorted desc above).
                if !seen_titles.insert(title.to_lowercase()) {
                    continue;
                }
                let busy = a.is_busy();
                let short_id = (!a.id.is_empty()).then(|| a.id.clone());
                let entry = SessionEntry {
                    id: rand_id("ses"),
                    title,
                    directory: dir.to_string(),
                    parent_id: String::new(),
                    created: now,
                    updated: now,
                    short_id,
                    claude_session_id: Some(a.session_id.clone()),
                    lineage: vec![a.session_id],
                    busy,
                    ..Default::default()
                };
                g.sessions.insert(entry.id.clone(), entry);
                changed = true;
            }
        }
        if changed {
            self.save();
        }
    }

    /// Find the opman session whose latest claude UUID matches, returning its id.
    #[allow(dead_code)] // available for future event-routing by claude uuid
    pub fn session_id_for_claude_uuid(&self, uuid: &str) -> Option<String> {
        self.reg
            .lock()
            .ok()?
            .by_claude_uuid(uuid)
            .map(|s| s.id.clone())
    }

    /// Mint a new opman session and announce it via `session.created`.
    pub fn create_session(&self, dir: &str, parent_id: &str, title: &str) -> SessionEntry {
        let now = now_ms();
        let entry = SessionEntry {
            id: rand_id("ses"),
            title: title.to_string(),
            directory: dir.to_string(),
            parent_id: parent_id.to_string(),
            created: now,
            updated: now,
            ..Default::default()
        };
        if let Ok(mut r) = self.reg.lock() {
            r.sessions.insert(entry.id.clone(), entry.clone());
        }
        self.save();
        self.emit(
            dir,
            "session.created",
            serde_json::json!({ "info": session_info(&entry) }),
        );
        entry
    }

    /// Register a child session row for a claude subagent so it nests under its parent
    /// in the sidebar and can be opened directly. The session `id` is the claude
    /// `agentId`. Idempotent; emits `session.created` only on first registration.
    pub fn ensure_subagent_session(&self, parent_id: &str, agent_id: &str, title: &str, dir: &str) {
        if agent_id.is_empty() {
            return;
        }
        let created = {
            let mut g = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if g.sessions.contains_key(agent_id) || g.deleted.contains(agent_id) {
                None
            } else {
                let now = now_ms();
                let entry = SessionEntry {
                    id: agent_id.to_string(),
                    title: if title.is_empty() {
                        "Subagent".to_string()
                    } else {
                        title.to_string()
                    },
                    directory: dir.to_string(),
                    parent_id: parent_id.to_string(),
                    created: now,
                    updated: now,
                    is_subagent: true,
                    ..Default::default()
                };
                g.sessions.insert(agent_id.to_string(), entry.clone());
                Some(entry)
            }
        };
        if let Some(entry) = created {
            self.save();
            self.emit(
                dir,
                "session.created",
                serde_json::json!({ "info": session_info(&entry) }),
            );
        }
    }

    /// Record a new background turn for a session (after `bg_start`/`bg_resume`).
    pub fn record_turn(
        self: &Arc<Self>,
        session_id: &str,
        short_id: String,
        claude_uuid: String,
        model: Option<String>,
    ) {
        let dir = {
            let mut guard = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(entry) = guard.sessions.get_mut(session_id) else {
                return;
            };
            entry.short_id = Some(short_id);
            if !claude_uuid.is_empty() {
                entry.claude_session_id = Some(claude_uuid.clone());
                if entry.lineage.last().map(|s| s.as_str()) != Some(claude_uuid.as_str()) {
                    entry.lineage.push(claude_uuid);
                }
            }
            if model.is_some() {
                entry.model = model;
            }
            entry.busy = true;
            entry.updated = now_ms();
            entry.directory.clone()
        };
        self.save();
        if let Some(entry) = self.get_session(session_id) {
            self.emit(
                &dir,
                "session.updated",
                serde_json::json!({ "info": session_info(&entry) }),
            );
            self.emit(
                &dir,
                "session.status",
                serde_json::json!({ "sessionID": session_id, "status": { "type": "busy" } }),
            );
        }
        self.clone().ensure_tailer(session_id);
    }

    /// Update a session's title and announce it. `manual` = a user rename (sticky:
    /// locks the title so the auto ai-title no longer overrides it).
    pub fn set_title(&self, session_id: &str, title: &str, manual: bool) {
        let dir = {
            let mut guard = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(entry) = guard.sessions.get_mut(session_id) else {
                return;
            };
            // A user rename is sticky; auto-titles never override a locked title.
            if !manual && entry.title_locked {
                return;
            }
            if entry.title == title && (entry.title_locked || !manual) {
                return;
            }
            entry.title = title.to_string();
            if manual {
                entry.title_locked = true;
            }
            entry.updated = now_ms();
            entry.directory.clone()
        };
        self.save();
        if let Some(entry) = self.get_session(session_id) {
            self.emit(
                &dir,
                "session.updated",
                serde_json::json!({ "info": session_info(&entry) }),
            );
        }
    }

    /// Remove a session from the registry and announce `session.deleted`.
    /// (Stopping the background agent is the caller's responsibility — it blocks.)
    pub fn remove_session(&self, session_id: &str) {
        let (dir, children) = {
            let mut g = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(e) = g.sessions.remove(session_id) else {
                return;
            };
            // Tombstone every claude UUID so import never resurrects this session.
            if let Some(c) = &e.claude_session_id {
                g.deleted.insert(c.clone());
            }
            for u in &e.lineage {
                g.deleted.insert(u.clone());
            }
            // Drop synthesized subagent children so they don't linger as orphans.
            let children: Vec<String> = g
                .sessions
                .values()
                .filter(|s| s.is_subagent && s.parent_id == session_id)
                .map(|s| s.id.clone())
                .collect();
            for c in &children {
                g.sessions.remove(c);
            }
            (e.directory, children)
        };
        if let Ok(mut t) = self.tailers.lock() {
            t.remove(session_id);
        }
        self.save();
        for c in &children {
            self.emit(
                &dir,
                "session.deleted",
                serde_json::json!({ "sessionID": c, "id": c }),
            );
        }
        self.emit(
            &dir,
            "session.deleted",
            serde_json::json!({ "sessionID": session_id, "id": session_id }),
        );
    }

    /// Set busy state; on a transition emit `session.status` (+ `session.idle`).
    /// Returns true when this call transitioned the session busy → idle (the signal the
    /// status poller uses to flush any queued follow-up prompt).
    pub fn set_busy(&self, session_id: &str, busy: bool) -> bool {
        let (dir, changed) = {
            let mut guard = match self.reg.lock() {
                Ok(g) => g,
                Err(_) => return false,
            };
            let Some(entry) = guard.sessions.get_mut(session_id) else {
                return false;
            };
            let changed = entry.busy != busy;
            entry.busy = busy;
            // Both edges of a turn are activity, so `updated` tracks the last
            // thing that happened in the session rather than only its start.
            if changed {
                entry.updated = now_ms();
            }
            (entry.directory.clone(), changed)
        };
        if !changed {
            return false;
        }
        self.save();
        let status = if busy { "busy" } else { "idle" };
        self.emit(
            &dir,
            "session.status",
            serde_json::json!({ "sessionID": session_id, "status": { "type": status } }),
        );
        if let Some(entry) = self.get_session(session_id) {
            self.emit(
                &dir,
                "session.updated",
                serde_json::json!({ "info": session_info(&entry) }),
            );
        }
        if !busy {
            self.emit(
                &dir,
                "session.idle",
                serde_json::json!({ "sessionID": session_id }),
            );
        }
        changed && !busy
    }

    /// Whether a session is busy or has a turn mid-dispatch (so a new prompt must queue
    /// rather than resume a live agent).
    pub fn is_occupied(&self, session_id: &str) -> bool {
        if self
            .dispatching
            .lock()
            .map(|d| d.contains(session_id))
            .unwrap_or(false)
        {
            return true;
        }
        self.get_session(session_id)
            .map(|s| s.busy)
            .unwrap_or(false)
    }

    /// Atomically reserve an idle session for a new turn.
    pub fn try_reserve_dispatch(&self, session_id: &str) -> bool {
        let Ok(mut dispatching) = self.dispatching.lock() else {
            return false;
        };
        if dispatching.contains(session_id) {
            return false;
        }
        if self
            .get_session(session_id)
            .map(|session| session.busy)
            .unwrap_or(false)
        {
            return false;
        }
        dispatching.insert(session_id.to_string());
        true
    }

    /// Record whether a session currently has an in-flight subagent (set by the tailer
    /// from the transcript). The poller ORs this into busy so the session stays alive
    /// while a subagent runs past the main agent's `state=done`.
    pub fn set_subagent_pending(&self, session_id: &str, pending: bool) {
        if let Ok(mut g) = self.reg.lock() {
            if let Some(e) = g.sessions.get_mut(session_id) {
                e.subagent_pending = pending;
            }
        }
    }

    /// Whether a session has an in-flight subagent (transcript-derived).
    pub fn subagent_pending(&self, session_id: &str) -> bool {
        self.get_session(session_id)
            .map(|s| s.subagent_pending)
            .unwrap_or(false)
    }

    /// Whether a session's turn is currently being spawned (poller must not reconcile it).
    pub fn is_dispatching(&self, session_id: &str) -> bool {
        self.dispatching
            .lock()
            .map(|d| d.contains(session_id))
            .unwrap_or(false)
    }

    /// Mark a session as just-aborted, starting its "settling" window.
    pub fn mark_aborting(&self, session_id: &str) {
        if let Ok(mut a) = self.aborting.lock() {
            a.insert(session_id.to_string(), now_ms());
        }
    }

    /// Stop tracking a session's abort (it's settled or superseded by a new turn).
    pub fn clear_aborting(&self, session_id: &str) {
        if let Ok(mut a) = self.aborting.lock() {
            a.remove(session_id);
        }
    }

    /// Whether an aborted session is still "settling": `claude stop` is graceful, so the
    /// agent can keep reporting busy for a beat. While settling, the poller should force
    /// the session idle rather than bounce it back to busy. Resolves (returns false and
    /// clears the mark) once the agent has actually gone idle or the safety cap elapses.
    pub fn abort_settling(&self, session_id: &str, agent_busy_now: bool) -> bool {
        let since = match self
            .aborting
            .lock()
            .ok()
            .and_then(|a| a.get(session_id).copied())
        {
            Some(s) => s,
            None => return false,
        };
        if !agent_busy_now || now_ms().saturating_sub(since) > ABORT_SETTLE_MS {
            self.clear_aborting(session_id);
            return false;
        }
        true
    }

    /// Queue a follow-up prompt to send once the session goes idle.
    pub fn enqueue_prompt(&self, session_id: &str, text: String) {
        if let Ok(mut q) = self.pending_prompts.lock() {
            q.entry(session_id.to_string()).or_default().push(text);
        }
    }

    /// Take all queued prompts for a session, joined into one resume turn (None if empty).
    pub fn take_pending(&self, session_id: &str) -> Option<String> {
        let mut q = self.pending_prompts.lock().ok()?;
        let v = q.remove(session_id)?;
        if v.is_empty() {
            None
        } else {
            Some(v.join("\n\n"))
        }
    }

    /// Drop any queued prompts (on abort/delete — the user no longer wants them sent).
    pub fn clear_pending(&self, session_id: &str) {
        if let Ok(mut q) = self.pending_prompts.lock() {
            q.remove(session_id);
        }
    }

    /// Snapshot the queued follow-up prompts for a session (oldest first).
    pub fn pending_list(&self, session_id: &str) -> Vec<String> {
        self.pending_prompts
            .lock()
            .ok()
            .and_then(|q| q.get(session_id).cloned())
            .unwrap_or_default()
    }

    /// Remove a single queued prompt by index. Returns true if one was removed.
    pub fn remove_pending(&self, session_id: &str, index: usize) -> bool {
        let Ok(mut q) = self.pending_prompts.lock() else {
            return false;
        };
        let Some(v) = q.get_mut(session_id) else {
            return false;
        };
        if index >= v.len() {
            return false;
        }
        v.remove(index);
        if v.is_empty() {
            q.remove(session_id);
        }
        true
    }

    /// Announce the current queued-prompt list for a session so the frontend's queue pill
    /// updates live (`session.queue` → `{ sessionID, pending: [...] }`).
    pub fn emit_queue_changed(&self, session_id: &str) {
        let Some(entry) = self.get_session(session_id) else {
            return;
        };
        let pending = self.pending_list(session_id);
        self.emit(
            &entry.directory,
            "session.queue",
            serde_json::json!({ "sessionID": session_id, "pending": pending }),
        );
    }

    /// Map of session id → busy, for `GET /session/status`.
    pub fn busy_map(&self) -> std::collections::HashMap<String, bool> {
        self.reg
            .lock()
            .map(|r| {
                r.sessions
                    .values()
                    .map(|s| (s.id.clone(), s.busy))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Build the `--settings` JSON for every `claude --bg` turn opman spawns. It
    /// registers opman's PreToolUse hook (`opman claude-hook`, through which
    /// permissions/questions route back here) and pins `worktree.bgIsolation` to
    /// `"none"`.
    ///
    /// The isolation pin is load-bearing for opman's whole model: by default a
    /// `claude --bg` agent moves into its own `.claude/worktrees/<name>/` checkout
    /// before editing, so each turn (and, in a kanban pipeline, each lane's fresh
    /// session) lands in a *different* worktree — stranding the prior stage's
    /// uncommitted work and hiding edits from opman's git/diff panels, which all
    /// operate on the session directory. opman always wants every session to edit
    /// the one shared working copy, so we declare that here rather than relying on
    /// a hand-placed (untracked) `.claude/settings.json` that may be absent for a
    /// given project path.
    fn hook_settings(&self) -> String {
        let cmd = format!("{} claude-hook", self.exe.to_string_lossy());
        serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "*", "hooks": [
                        // `timeout` (seconds) must exceed claude's 600s default so the hook
                        // keeps blocking while a human answers a permission/question in the
                        // web UI. Without it claude cancels the hook at 600s and runs the
                        // tool natively (AskUserQuestion then prompts in the attached
                        // terminal instead of surfacing in opman).
                        { "type": "command", "command": cmd, "timeout": 3600 }
                    ] }
                ]
            },
            "worktree": { "bgIsolation": "none" }
        })
        .to_string()
    }

    /// Assemble the per-turn `claude` options for a session.
    fn build_opts(&self, session_id: &str, dir: &str) -> claude_cli::TurnOpts {
        claude_cli::TurnOpts {
            model: self
                .get_session(session_id)
                .and_then(|s| s.model)
                .or_else(default_model),
            // Resolve through the alias/validation map so a previously-persisted opencode
            // name (e.g. "build") never reaches claude as a bogus `--agent`.
            agent: self
                .get_session(session_id)
                .and_then(|s| s.agent)
                .map(|a| self.resolve_agent(session_id, &a))
                .filter(|a| !a.is_empty()),
            effort: self.get_session(session_id).and_then(|s| s.effort),
            permission_mode: self.effective_mode(session_id),
            settings_json: self.hook_settings(),
            engine_url: self.url(),
            mcp_config: self.mcp_config_json(dir, session_id).unwrap_or_default(),
            session_env_id: session_id.to_string(),
        }
    }

    /// Spawn a background `claude` turn for a session (new agent, or `--resume` of the
    /// latest lineage UUID) and record it on completion. Marks the session busy and
    /// "dispatching" up front so a concurrent send queues instead of racing a second
    /// agent onto the same conversation.
    pub fn spawn_turn(self: &Arc<Self>, session_id: String, text: String) {
        self.spawn_turn_internal(session_id, text, None);
    }

    /// Spawn a queued follow-up. Resume failures are retried a small number of times so
    /// a transient Claude CLI handoff cannot silently strand the user's prompt.
    pub fn spawn_queued_turn(self: &Arc<Self>, session_id: String, text: String) {
        self.spawn_turn_internal(session_id, text, Some(0));
    }

    fn spawn_turn_internal(
        self: &Arc<Self>,
        session_id: String,
        text: String,
        queued_attempt: Option<u8>,
    ) {
        if text.trim().is_empty() {
            return;
        }
        let Some(entry) = self.get_session(&session_id) else {
            return;
        };
        let dir = entry.directory.clone();
        let resume = entry.claude_session_id.clone();
        let opts = self.build_opts(&session_id, &dir);
        let turn_text = text.clone();

        // A fresh turn supersedes any pending abort-settling for this session.
        self.clear_aborting(&session_id);

        // Guard against a racing dispatch and an over-eager poller until the agent
        // registers; record_turn (or the failure path) clears these.
        // Normal dispatches reserve this before calling spawn_turn. Keep this
        // insertion for internal callers that invoke spawn_turn directly.
        if let Ok(mut d) = self.dispatching.lock() {
            d.insert(session_id.clone());
        }
        self.set_busy(&session_id, true);

        let engine = self.clone();
        tokio::spawn(async move {
            let sid = session_id.clone();
            let result = tokio::task::spawn_blocking(move || match resume {
                Some(uuid) if !uuid.is_empty() => {
                    claude_cli::bg_resume(&dir, &uuid, &opts, &turn_text)
                }
                _ => claude_cli::bg_start(&dir, &opts, &turn_text),
            })
            .await;
            match result {
                Ok(Ok((short_id, uuid))) => {
                    let model = engine.get_session(&sid).and_then(|s| s.model);
                    engine.record_turn(&sid, short_id, uuid, model);
                }
                Ok(Err(e)) => {
                    tracing::warn!("claude turn failed: {e}");
                    engine.emit_system(
                        &sid,
                        "error",
                        &format!("Failed to start the claude turn: {e}"),
                    );
                    engine.finish_turn_failure(&sid, text, queued_attempt);
                }
                Err(e) => {
                    tracing::warn!("claude turn join error: {e}");
                    engine.emit_system(&sid, "error", &format!("claude turn crashed: {e}"));
                    engine.finish_turn_failure(&sid, text, queued_attempt);
                }
            }
            if let Ok(mut d) = engine.dispatching.lock() {
                d.remove(&sid);
            }
        });
    }

    fn finish_turn_failure(self: &Arc<Self>, session_id: &str, text: String, attempt: Option<u8>) {
        if let Ok(mut dispatching) = self.dispatching.lock() {
            dispatching.remove(session_id);
        }
        self.set_busy(session_id, false);

        let Some(attempt) = attempt else { return };
        if attempt >= 2 {
            return;
        }

        self.enqueue_prompt(session_id, text);
        self.emit_queue_changed(session_id);
        let engine = self.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if !engine.try_reserve_dispatch(&sid) {
                return;
            }
            let Some(next) = engine.take_pending(&sid) else {
                if let Ok(mut dispatching) = engine.dispatching.lock() {
                    dispatching.remove(&sid);
                }
                return;
            };
            engine.emit_queue_changed(&sid);
            engine.spawn_turn_internal(sid, next, Some(attempt + 1));
        });
    }

    fn ensure_tailer(self: Arc<Self>, session_id: &str) {
        {
            let mut t = match self.tailers.lock() {
                Ok(t) => t,
                Err(_) => return,
            };
            if !t.insert(session_id.to_string()) {
                return; // already running
            }
        }
        tailer::spawn_tailer(self, session_id.to_string());
    }
}

/// PreToolUse hook entry point (`opman claude-hook`).
///
/// Reads the hook payload from stdin, relays it to the running engine's
/// `/internal/ask` endpoint (resolved from `OPMAN_ENGINE_URL`), and prints the
/// permission-decision JSON the `claude` agent expects. Fails open (allow) on any
/// error so a hiccup never wedges an agent.
pub async fn run_permission_hook() -> Result<()> {
    use std::io::Read;

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        println!("{}", allow_decision());
        return Ok(());
    }
    let payload: serde_json::Value =
        serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);
    let engine_url = std::env::var("OPMAN_ENGINE_URL").ok();
    println!("{}", permission_hook_reply(payload, engine_url).await);
    Ok(())
}

/// The "allow" permission-decision JSON the `claude` agent expects (fail-open default).
fn allow_decision() -> String {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow"
        }
    })
    .to_string()
}

/// Core of the PreToolUse hook: relay the parsed hook `payload` to the engine's
/// `/internal/ask` endpoint (from `OPMAN_ENGINE_URL`) and return the permission-decision
/// JSON to print. Extracted from [`run_permission_hook`] so the relay logic is unit-
/// testable without stdin. Fails open (allow) on a missing/empty URL or any relay error.
async fn permission_hook_reply(payload: serde_json::Value, engine_url: Option<String>) -> String {
    let url = match engine_url {
        Some(u) if !u.is_empty() => u,
        _ => return allow_decision(),
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{url}/internal/ask"))
        .json(&payload)
        // Generous: a human may take a while to answer a permission/question prompt.
        .timeout(std::time::Duration::from_secs(3600))
        .send()
        .await;

    match resp {
        Ok(r) => match r.text().await {
            Ok(body) if !body.trim().is_empty() => body,
            _ => allow_decision(),
        },
        Err(_) => allow_decision(),
    }
}

/// Build the opencode `session.info` object opman/web expect.
fn session_info(entry: &SessionEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "title": entry.title,
        "parentID": entry.parent_id,
        "directory": entry.directory,
        "time": { "created": entry.created, "updated": entry.updated },
    })
}

/// Start the embedded adapter server. Mirrors `server::spawn_agent_server`'s
/// `(base_url, ServerHandle)` return so `main.rs` can swap cleanly.
pub async fn start_embedded_server(
    mcp: crate::mcp_registry::SharedRegistry,
) -> Result<(String, ServerHandle)> {
    let persist = dirs::config_dir().map(|d| d.join("opman").join("claude_sessions.json"));
    let engine = Arc::new(ClaudeEngine::new(persist, mcp));
    let _ = ENGINE.set(engine.clone());

    // Fetch available models once at startup in a background task so the
    // /provider route is ready without blocking server startup.
    {
        let eng = engine.clone();
        tokio::task::spawn(async move {
            if let Some(models) = tokio::task::spawn_blocking(claude_cli::fetch_models_via_cli)
                .await
                .ok()
                .flatten()
            {
                eng.set_cached_models(models);
            }
        });
    }

    // Background poller: reconcile busy/idle from `claude agents --json`.
    tailer::spawn_status_poller(engine.clone());

    // Background reaper: stop finished/untracked/stale-idle background agents so their
    // warm claude daemon/spare processes don't accumulate and slow the host over time.
    reaper::spawn_reaper(engine.clone());

    let app = routes::router(engine.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind embedded claude engine port")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}");
    engine.set_url(&url);
    info!(%url, "Claude engine (embedded opencode adapter) ready");

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Claude engine server error: {e}");
        }
    });

    // No external child process to manage; shutdown is implicit on exit.
    let handle: ServerHandle = Arc::new(std::sync::Mutex::new(None));
    Ok((url, handle))
}

#[cfg(test)]
#[path = "engine_state_tests.rs"]
mod engine_state_tests;

#[cfg(test)]
#[path = "engine_lifecycle_tests.rs"]
mod engine_lifecycle_tests;

#[cfg(test)]
#[path = "import_agents_tests.rs"]
mod import_agents_tests;

#[cfg(test)]
#[path = "permission_hook_tests.rs"]
mod permission_hook_tests;

#[cfg(test)]
#[path = "spawn_turn_tests.rs"]
mod spawn_turn_tests;

#[cfg(test)]
#[path = "embedded_server_tests.rs"]
mod embedded_server_tests;

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    fn engine() -> Arc<ClaudeEngine> {
        Arc::new(ClaudeEngine::new(
            None,
            crate::mcp_registry::RegistryHandle::default(),
        ))
    }

    // The core of the resume-safety fix: while an agent is running, a follow-up must
    // queue (not resume a live agent); it flushes on the busy → idle transition.
    #[test]
    fn busy_session_queues_followups_and_flushes_on_idle() {
        let e = engine();
        let s = e.create_session("/tmp/proj", "", "t");
        let id = s.id.clone();

        // Idle, empty queue → not occupied, nothing to flush.
        assert!(!e.is_occupied(&id));
        assert!(e.take_pending(&id).is_none());

        // Goes busy → occupied; follow-ups queue.
        assert!(!e.set_busy(&id, true)); // busy edge is not the idle edge
        assert!(e.is_occupied(&id));
        e.enqueue_prompt(&id, "first".into());
        e.enqueue_prompt(&id, "second".into());

        // The idle edge is reported once; queued prompts join into one resume turn.
        assert!(e.set_busy(&id, false));
        assert!(!e.set_busy(&id, false)); // no second edge
        assert_eq!(e.take_pending(&id).as_deref(), Some("first\n\nsecond"));
        assert!(e.take_pending(&id).is_none());
    }

    #[test]
    fn dispatching_guard_marks_occupied() {
        let e = engine();
        let s = e.create_session("/tmp/proj", "", "t");
        let id = s.id.clone();
        assert!(!e.is_occupied(&id));
        e.dispatching.lock().unwrap().insert(id.clone());
        assert!(e.is_dispatching(&id));
        assert!(e.is_occupied(&id)); // mid-dispatch counts as occupied
    }

    #[test]
    fn subagent_session_registers_as_child_and_cleans_up() {
        let e = engine();
        let parent = e.create_session("/tmp/proj", "", "parent");
        let pid = parent.id.clone();

        // First call registers a child keyed by the agentId, with parentID = parent.
        e.ensure_subagent_session(&pid, "agent_abc", "Count files", "/tmp/proj");
        let sub = e.get_session("agent_abc").expect("subagent registered");
        assert!(sub.is_subagent);
        assert_eq!(sub.parent_id, pid);
        assert_eq!(sub.title, "Count files");
        assert_eq!(e.list_for_dir("/tmp/proj").len(), 2); // parent + child

        // Idempotent — no duplicate.
        e.ensure_subagent_session(&pid, "agent_abc", "Count files", "/tmp/proj");
        assert_eq!(e.list_for_dir("/tmp/proj").len(), 2);

        // Deleting the parent removes its synthesized children too.
        e.remove_session(&pid);
        assert!(e.get_session("agent_abc").is_none());
        assert!(e.list_for_dir("/tmp/proj").is_empty());
    }

    #[test]
    fn agent_resolution_validates_against_real_claude_agents() {
        let e = engine();
        let s = e.create_session("/proj", "", "t");
        // Real agent list as claude's init event would report it.
        e.set_cached_init(
            "/proj",
            claude_cli::InitInfo {
                commands: vec![],
                agents: vec![
                    "claude".into(),
                    "Explore".into(),
                    "general-purpose".into(),
                    "Plan".into(),
                ],
            },
        );
        // opencode aliases translate; case-insensitive real names normalize.
        e.set_agent(&s.id, "plan");
        assert_eq!(e.get_session(&s.id).unwrap().agent.as_deref(), Some("Plan"));
        e.set_agent(&s.id, "explore");
        assert_eq!(
            e.get_session(&s.id).unwrap().agent.as_deref(),
            Some("Explore")
        );
        // build / unknown / reviewer have no claude equivalent → default (cleared).
        e.set_agent(&s.id, "build");
        assert_eq!(e.get_session(&s.id).unwrap().agent, None);
        e.set_agent(&s.id, "code-reviewer");
        assert_eq!(e.get_session(&s.id).unwrap().agent, None);
    }

    #[test]
    fn clear_pending_drops_queue() {
        let e = engine();
        let s = e.create_session("/tmp/proj", "", "t");
        let id = s.id.clone();
        e.enqueue_prompt(&id, "x".into());
        e.clear_pending(&id);
        assert!(e.take_pending(&id).is_none());
    }
}
