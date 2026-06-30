//! opencode-compatible REST + SSE routes, backed by the Claude engine.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tracing::debug;

use super::{claude_cli, jsonl, ClaudeEngine};

type Engine = Arc<ClaudeEngine>;

pub fn router(engine: Engine) -> Router {
    Router::new()
        .route("/info", get(info))
        .route("/health", get(health))
        .route("/event", get(event_stream))
        .route("/session", get(list_sessions).post(create_session))
        .route("/session/status", get(session_status))
        .route("/provider", get(provider))
        .route("/command", get(command_list))
        .route("/agent", get(agent_list))
        .route(
            "/session/{id}",
            get(get_session).patch(rename_session).delete(delete_session),
        )
        .route("/session/{id}/message", get(get_messages).post(send_message))
        .route("/session/{id}/prompt_async", post(prompt_async))
        .route("/session/{id}/abort", post(abort))
        .route("/session/{id}/todo", get(get_todos))
        .route("/session/{id}/command", post(session_command))
        .route("/session/{id}/revert", post(noop_ok))
        .route("/session/{id}/unrevert", post(noop_ok))
        .route("/session/{id}/share", post(noop_obj))
        .route("/tui/select-session", post(select_session))
        .route("/permission/{id}/reply", post(permission_reply))
        .route("/question/{id}/reply", post(question_reply))
        .route("/question/{id}/reject", post(question_reject))
        // Internal: PreToolUse hook callback (opman claude-hook → adapter).
        .route("/internal/ask", post(internal_ask))
        .with_state(engine)
}

// ── helpers ─────────────────────────────────────────────────────────

fn dir_header(headers: &HeaderMap) -> String {
    headers
        .get("x-opencode-directory")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn session_obj(entry: &super::registry::SessionEntry) -> Value {
    json!({
        "id": entry.id,
        "slug": "",
        "title": entry.title,
        "version": claude_cli::version(),
        "projectID": "claude",
        "parentID": entry.parent_id,
        "directory": entry.directory,
        "time": { "created": entry.created, "updated": entry.updated },
    })
}

fn extract_text(body: &Value) -> String {
    if let Some(parts) = body.get("parts").and_then(|p| p.as_array()) {
        let joined = parts
            .iter()
            .filter_map(|p| {
                let t = p.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                if t == "text" {
                    p.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !joined.is_empty() {
            return joined;
        }
    }
    // Fallbacks for simpler payloads.
    body.get("text")
        .and_then(|t| t.as_str())
        .or_else(|| body.get("prompt").and_then(|t| t.as_str()))
        .unwrap_or("")
        .to_string()
}

/// Valid claude permission modes (for the runtime `/permission-mode` command).
const PERMISSION_MODES: &[&str] = &[
    "default",
    "acceptEdits",
    "auto",
    "bypassPermissions",
    "dontAsk",
    "plan",
];

/// If `text` is a runtime control command (`/permission-mode <mode>`), apply it and
/// return true (so no claude turn is dispatched).
fn handle_control_command(engine: &Engine, session_id: &str, text: &str) -> bool {
    let t = text.trim();

    // `/agent <name>` switches the session's claude agent. In opencode this is a server
    // slash command; the claude engine applies it as the `--agent` flag on the next
    // turn. Intercept it here so we never send a literal "/agent <name>" prompt to
    // claude — claude has no such command and would no-op the whole turn with a
    // synthetic "No response requested.", stalling the session.
    if t == "/agent" || t == "/agents" {
        return true; // bare form: nothing to switch, and don't forward to claude
    }
    if let Some(name) = t.strip_prefix("/agent ") {
        let name = name.trim();
        if !name.is_empty() {
            engine.set_agent(session_id, name);
            if let Some(entry) = engine.get_session(session_id) {
                engine.emit(
                    &entry.directory,
                    "tui.toast.show",
                    json!({ "message": format!("Claude agent: {name}"), "variant": "info" }),
                );
            }
        }
        return true;
    }

    let rest = t
        .strip_prefix("/permission-mode")
        .or_else(|| t.strip_prefix("/perm-mode"))
        .or_else(|| t.strip_prefix("/perm"));
    if let Some(rest) = rest {
        let mode = rest.trim();
        let resolved = PERMISSION_MODES
            .iter()
            .find(|m| m.eq_ignore_ascii_case(mode))
            .copied();
        match resolved {
            Some(m) => engine.set_permission_mode(session_id, m),
            None => {
                if let Some(entry) = engine.get_session(session_id) {
                    engine.emit(
                        &entry.directory,
                        "tui.toast.show",
                        json!({
                            "message": format!("Unknown permission mode '{mode}'. Options: {}", PERMISSION_MODES.join(", ")),
                            "variant": "error"
                        }),
                    );
                }
            }
        }
        return true;
    }
    false
}

/// Dispatch a user turn. If the session's agent is still running (or a turn is
/// mid-dispatch), the prompt is queued and sent as a single `--resume` turn once the
/// session goes fully idle — opman never resumes a live agent, which would spawn a
/// competing process and orphan its in-flight subagents. The status poller flushes the
/// queue on the busy → idle transition.
fn dispatch_turn(engine: Engine, session_id: String, text: String) {
    if text.trim().is_empty() {
        return;
    }
    if handle_control_command(&engine, &session_id, &text) {
        return;
    }
    let Some(entry) = engine.get_session(&session_id) else {
        return;
    };
    if engine.is_occupied(&session_id) {
        engine.enqueue_prompt(&session_id, text);
        engine.emit(
            &entry.directory,
            "tui.toast.show",
            json!({ "message": "Queued — will send when the agent is free.", "variant": "info" }),
        );
        return;
    }
    engine.spawn_turn(session_id, text);
}

// ── handlers ────────────────────────────────────────────────────────

async fn info(headers: HeaderMap) -> Json<Value> {
    Json(json!({ "directory": dir_header(&headers), "version": claude_cli::version() }))
}

async fn health() -> impl IntoResponse {
    "ok"
}

async fn list_sessions(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    // Import any existing `claude` sessions for this directory so previous
    // conversations show up in the sidebar.
    if !dir.is_empty() {
        let d = dir.clone();
        if let Ok(Ok(agents)) =
            tokio::task::spawn_blocking(move || claude_cli::agents_json(Some(&d))).await
        {
            engine.import_agents(&dir, agents);
        }
    }
    let arr: Vec<Value> = engine.list_for_dir(&dir).iter().map(session_obj).collect();
    Json(Value::Array(arr))
}

async fn create_session(
    State(engine): State<Engine>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let dir = dir_header(&headers);
    let body = body.map(|b| b.0).unwrap_or(Value::Null);
    let parent = body
        .get("parentID")
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string();
    let title = body
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("New session")
        .to_string();
    let entry = engine.create_session(&dir, &parent, &title);
    Json(session_obj(&entry))
}

async fn get_session(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    match engine.get_session(&id) {
        Some(entry) => Json(session_obj(&entry)),
        None => Json(json!({ "id": id })),
    }
}

async fn rename_session(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    if let Some(title) = body.get("title").and_then(|t| t.as_str()) {
        engine.set_title(&id, title, true);
    }
    match engine.get_session(&id) {
        Some(entry) => Json(session_obj(&entry)),
        None => Json(json!({ "id": id })),
    }
}

async fn delete_session(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    engine.clear_pending(&id);
    // Stop the background agent (best effort), then drop it from the registry.
    if let Some(short) = engine.get_session(&id).and_then(|s| s.short_id) {
        let _ = tokio::task::spawn_blocking(move || claude_cli::stop(&short)).await;
    }
    engine.remove_session(&id);
    Json(json!({ "ok": true }))
}

async fn session_status(State(engine): State<Engine>) -> Json<Value> {
    // Only busy sessions are present (idle ones are absent), matching opencode.
    let mut map = serde_json::Map::new();
    for (id, busy) in engine.busy_map() {
        if busy {
            map.insert(id, json!({ "type": "busy" }));
        }
    }
    Json(Value::Object(map))
}

async fn get_messages(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    let Some(entry) = engine.get_session(&id) else {
        // Not an opman session — it may be a subagent child id (the web UI fetches
        // `/session/<agentId>/message` to backfill a completed task on reload).
        if let Some(path) = claude_cli::locate_subagent_jsonl(&id) {
            let parsed = jsonl::parse_file(&path, &id);
            let arr: Vec<Value> = parsed.messages.iter().map(|m| m.to_value()).collect();
            return Json(Value::Array(arr));
        }
        return Json(Value::Array(vec![]));
    };
    // A subagent child session: its transcript is the agent-<id>.jsonl, located by id.
    if entry.is_subagent {
        if let Some(path) = claude_cli::locate_subagent_jsonl(&entry.id) {
            let parsed = jsonl::parse_file(&path, &entry.id);
            let arr: Vec<Value> = parsed.messages.iter().map(|m| m.to_value()).collect();
            return Json(Value::Array(arr));
        }
        return Json(Value::Array(vec![]));
    }
    // A `--bg --resume` turn writes a *fresh* transcript (with full history), so the
    // latest UUID's file may be missing/empty for a moment mid-turn. Walk the lineage
    // newest→oldest and return the first non-empty transcript so the conversation
    // never transiently disappears during a follow-up.
    let mut uuids: Vec<String> = Vec::new();
    if let Some(c) = &entry.claude_session_id {
        uuids.push(c.clone());
    }
    for u in entry.lineage.iter().rev() {
        if !uuids.contains(u) {
            uuids.push(u.clone());
        }
    }
    for uuid in uuids {
        if let Some(path) = claude_cli::locate_jsonl(&uuid) {
            let mut parsed = jsonl::parse_file(&path, &entry.id);
            if !parsed.messages.is_empty() {
                jsonl::enrich_subagents(&mut parsed);
                jsonl::enrich_background_tasks(&mut parsed);
                let arr: Vec<Value> = parsed.messages.iter().map(|m| m.to_value()).collect();
                return Json(Value::Array(arr));
            }
        }
    }
    Json(Value::Array(vec![]))
}

async fn send_message(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    // The web UI may include a selected model `{ providerID, modelID }`.
    if let Some(model_id) = body
        .get("model")
        .and_then(|m| m.get("modelID"))
        .and_then(|s| s.as_str())
    {
        engine.set_model(&id, model_id);
    }
    // The kanban launch (and agent mentions) may include a selected agent.
    if let Some(agent) = body.get("agent").and_then(|a| a.as_str()) {
        engine.set_agent(&id, agent);
    }
    let text = extract_text(&body.0);
    debug!(session = %id, "claude engine: send_message");
    dispatch_turn(engine, id, text);
    Json(json!({ "ok": true }))
}

async fn prompt_async(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let text = extract_text(&body.0);
    dispatch_turn(engine, id, text);
    Json(json!({ "ok": true }))
}

async fn session_command(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let command = body.get("command").and_then(|c| c.as_str()).unwrap_or("");
    let args = body.get("arguments").and_then(|a| a.as_str()).unwrap_or("");
    let text = if args.is_empty() {
        format!("/{command}")
    } else {
        format!("/{command} {args}")
    };
    dispatch_turn(engine, id, text);
    Json(json!({ "ok": true }))
}

async fn abort(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    // User-initiated stop: drop any queued follow-ups so they aren't auto-sent later.
    engine.clear_pending(&id);
    // Don't let a (now-killed) subagent's still-fresh transcript keep the session busy.
    engine.set_subagent_pending(&id, false);
    // Enter the abort "settling" window BEFORE stopping: `claude stop` is graceful, so the
    // agent may report `working` for another poll or two — the poller force-idles the
    // session while it settles so it doesn't visibly bounce back to busy.
    engine.mark_aborting(&id);
    engine.set_busy(&id, false);
    if let Some(short) = engine.get_session(&id).and_then(|s| s.short_id) {
        let _ = tokio::task::spawn_blocking(move || claude_cli::stop(&short)).await;
    }
    Json(json!({ "ok": true }))
}

async fn get_todos(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    let mut todos: Vec<Value> = vec![];
    if let Some(entry) = engine.get_session(&id) {
        if let Some(uuid) = entry.claude_session_id {
            if let Some(path) = claude_cli::locate_jsonl(&uuid) {
                let parsed = jsonl::parse_file(&path, &entry.id);
                // Latest TodoWrite tool call wins.
                for msg in &parsed.messages {
                    for part in &msg.parts {
                        if part.get("tool").and_then(|t| t.as_str()) == Some("TodoWrite") {
                            if let Some(items) = part
                                .get("state")
                                .and_then(|s| s.get("input"))
                                .and_then(|i| i.get("todos"))
                                .and_then(|t| t.as_array())
                            {
                                todos = items
                                    .iter()
                                    .map(|t| {
                                        json!({
                                            "content": t.get("content").and_then(|c| c.as_str()).unwrap_or(""),
                                            "status": t.get("status").and_then(|s| s.as_str()).unwrap_or("pending"),
                                            "priority": t.get("priority").and_then(|p| p.as_str()).unwrap_or(""),
                                        })
                                    })
                                    .collect();
                            }
                        }
                    }
                }
            }
        }
    }
    Json(Value::Array(todos))
}

async fn provider() -> Json<Value> {
    // Synthetic provider list in opencode's shape: { all, connected, default }.
    // The web model picker reads `all[].models` (keyed by modelID) and `default`.
    // `limit.context` is the model's input context window; `limit.output` its max
    // output tokens — per-model, not a single shared value (these drive the token
    // budget the UI shows). Values match the current Claude model catalog.
    let model = |id: &str, name: &str, context: u64, output: u64| {
        json!({
            "id": id,
            "providerID": "anthropic",
            "name": name,
            "limit": { "context": context, "output": output },
        })
    };
    Json(json!({
        "all": [
            {
                "id": "anthropic",
                "name": "Anthropic",
                "models": {
                    "claude-opus-4-8": model("claude-opus-4-8", "Claude Opus 4.8", 1_000_000, 128_000),
                    "claude-sonnet-4-6": model("claude-sonnet-4-6", "Claude Sonnet 4.6", 1_000_000, 64_000),
                    "claude-haiku-4-5-20251001": model("claude-haiku-4-5-20251001", "Claude Haiku 4.5", 200_000, 64_000),
                }
            }
        ],
        "connected": ["anthropic"],
        "default": { "anthropic": "claude-sonnet-4-6" },
    }))
}

/// Friendly descriptions for well-known claude built-in slash commands. Skills and
/// custom commands (not listed here) fall back to a generic label.
fn command_description(name: &str) -> &'static str {
    match name {
        "compact" => "Compact the conversation to save context",
        "clear" => "Clear the conversation history",
        "context" => "Show context window usage",
        "init" => "Generate a CLAUDE.md for this project",
        "review" => "Review the current changes",
        "security-review" => "Security review of the pending changes",
        "config" => "Open configuration",
        "usage" => "Show usage and limits",
        "usage-credits" => "Show usage credits",
        "extra-usage" => "Show extra usage",
        "insights" => "Show session insights",
        "goal" => "Set or show the session goal",
        "reload-skills" => "Reload skills",
        "heapdump" => "Capture a heap dump (debug)",
        "deep-research" => "Run a deep multi-source research report",
        "code-review" => "Review the current diff for bugs and cleanups",
        "simplify" => "Simplify the changed code",
        "verify" => "Verify a change by running the app",
        "debug" => "Investigate a hard bug",
        "loop" => "Run a prompt on a recurring interval",
        "schedule" => "Manage scheduled cloud agents",
        "claude-api" => "Claude API / SDK reference",
        "run" => "Launch and drive the project's app",
        "batch" => "Batch-process a list of items",
        "fewer-permission-prompts" => "Reduce permission prompts",
        "update-config" => "Configure the Claude Code harness",
        "design-sync" => "Sync design tokens",
        "run-skill-generator" => "Generate a new skill",
        "team-onboarding" => "Generate a team onboarding guide",
        _ => "",
    }
}

/// Fetch claude's init introspection (slash commands + agents) for a directory,
/// caching it after the first (subprocess) call.
async fn init_for_dir(engine: &Engine, dir: &str) -> claude_cli::InitInfo {
    if let Some(info) = engine.cached_init(dir) {
        return info;
    }
    let d = dir.to_string();
    let info = tokio::task::spawn_blocking(move || claude_cli::introspect(&d))
        .await
        .unwrap_or_default();
    engine.set_cached_init(dir, info.clone());
    info
}

async fn command_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    // Discover claude's slash commands for this directory (cached after first call).
    let commands = init_for_dir(&engine, &dir).await.commands;
    let arr: Vec<Value> = commands
        .iter()
        .map(|name| {
            let desc = command_description(name);
            if desc.is_empty() {
                json!({ "name": name })
            } else {
                json!({ "name": name, "description": desc })
            }
        })
        .collect();
    Json(Value::Array(arr))
}

/// Friendly descriptions for well-known claude built-in agents.
fn agent_description(name: &str) -> &'static str {
    match name {
        "claude" => "Default agent for general tasks",
        "general-purpose" => "Researches complex questions and runs multi-step tasks",
        "Explore" => "Fast read-only codebase search and exploration",
        "Plan" => "Designs implementation plans before coding",
        "statusline-setup" => "Configures the status line",
        "claude-code-guide" => "Answers Claude Code / API / SDK questions",
        _ => "",
    }
}

/// GET /agent — the real claude agents for this directory (built-ins + project/user
/// agents), read from claude's `system/init` event. The opman web layer proxies this
/// for its agent picker; without it the picker falls back to opencode's `build`/`plan`.
async fn agent_list(State(engine): State<Engine>, headers: HeaderMap) -> Json<Value> {
    let dir = dir_header(&headers);
    if dir.is_empty() {
        return Json(Value::Array(vec![]));
    }
    let agents = init_for_dir(&engine, &dir).await.agents;
    let arr: Vec<Value> = agents
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "description": agent_description(name),
                // `claude` is the default primary agent; the rest are selectable too.
                // Nothing is "subagent" (which the picker would hide).
                "mode": if name == "claude" { "primary" } else { "all" },
                "native": true,
            })
        })
        .collect();
    Json(Value::Array(arr))
}

async fn select_session(body: Option<Json<Value>>) -> Json<Value> {
    let _ = body;
    Json(json!({ "ok": true }))
}

async fn noop_ok() -> Json<Value> {
    Json(json!({ "ok": true }))
}

// ── permissions & questions (hook ⇄ opman) ──────────────────────────

/// Tools that require user approval (everything else is auto-allowed).
const GATED_TOOLS: &[&str] = &["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"];
const EDIT_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];

fn hook_allow() -> Value {
    json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "allow" } })
}

fn hook_deny(reason: &str) -> Value {
    json!({ "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason,
    } })
}

/// PreToolUse hook callback. The hook (a thin `opman claude-hook` relay) posts the
/// raw hook input here; we decide allow/deny — surfacing `permission.asked` /
/// `question.asked` to opman and blocking until the user replies when needed.
async fn internal_ask(State(engine): State<Engine>, body: Json<Value>) -> Json<Value> {
    let input = body.0;
    let claude_uuid = input.get("session_id").and_then(|s| s.as_str()).unwrap_or("");
    let cwd = input.get("cwd").and_then(|s| s.as_str()).unwrap_or("");
    let tool = input.get("tool_name").and_then(|s| s.as_str()).unwrap_or("");
    let tool_input = input.get("tool_input").cloned().unwrap_or(json!({}));

    // Resolve the opman session: by claude uuid, else newest in cwd.
    let session_id = engine
        .session_id_for_claude_uuid(claude_uuid)
        .or_else(|| engine.list_for_dir(cwd).into_iter().next().map(|s| s.id));
    let Some(session_id) = session_id else {
        return Json(hook_allow()); // unknown session → fail open
    };
    let dir = engine
        .get_session(&session_id)
        .map(|s| s.directory)
        .unwrap_or_else(|| cwd.to_string());
    let mode = engine.effective_mode(&session_id);

    // AskUserQuestion → surface as a question, answer via the deny-reason channel.
    if tool == "AskUserQuestion" {
        let id = rand_request_id();
        let questions = build_questions(&tool_input, &session_id);
        engine.emit(
            &dir,
            "question.asked",
            json!({ "id": id, "sessionID": session_id, "questions": questions }),
        );
        let rx = engine.register_pending(&id);
        match tokio::time::timeout(Duration::from_secs(3600), rx).await {
            Ok(Ok(super::PendingReply::Question(answers))) => {
                let reason = format_answers(&tool_input, &answers);
                return Json(hook_deny(&reason));
            }
            _ => {
                engine.resolve_pending(&id, super::PendingReply::Reject);
                return Json(hook_deny(
                    "[USER] The question was dismissed without an answer. Make a reasonable default choice and continue.",
                ));
            }
        }
    }

    // Permission-gated tools.
    let gated = GATED_TOOLS.contains(&tool);
    if mode == "bypassPermissions" || !gated {
        return Json(hook_allow());
    }
    if mode == "acceptEdits" && EDIT_TOOLS.contains(&tool) {
        return Json(hook_allow());
    }
    if mode == "plan" && EDIT_TOOLS.contains(&tool) {
        return Json(hook_deny(
            "[USER] Plan mode is active — do not modify files. Describe the plan instead.",
        ));
    }
    if engine.is_always_allowed(&session_id, tool) {
        return Json(hook_allow());
    }

    // Ask opman for approval and block until the user replies.
    let id = rand_request_id();
    let patterns = permission_patterns(&tool_input);
    engine.emit(
        &dir,
        "permission.asked",
        json!({
            "id": id,
            "sessionID": session_id,
            "permission": tool,
            "patterns": patterns,
            "metadata": tool_input,
        }),
    );
    let rx = engine.register_pending(&id);
    match tokio::time::timeout(Duration::from_secs(3600), rx).await {
        Ok(Ok(super::PendingReply::Permission(reply))) => match reply.as_str() {
            "always" => {
                engine.add_allowed_tool(&session_id, tool);
                Json(hook_allow())
            }
            "reject" => Json(hook_deny("[USER] Permission denied by the user.")),
            _ => Json(hook_allow()), // "once"
        },
        _ => {
            engine.resolve_pending(&id, super::PendingReply::Reject);
            Json(hook_deny("[USER] Permission request was not answered."))
        }
    }
}

fn rand_request_id() -> String {
    let n: u128 = rand::random();
    format!("perm_{n:032x}")
}

/// Extract candidate file patterns from a tool input for the permission card.
fn permission_patterns(tool_input: &Value) -> Vec<String> {
    let mut out = vec![];
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(p) = tool_input.get(key).and_then(|v| v.as_str()) {
            out.push(p.to_string());
        }
    }
    if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
        out.push(cmd.to_string());
    }
    out
}

/// Convert claude's AskUserQuestion input into opman's QuestionRequest shape.
fn build_questions(tool_input: &Value, _session_id: &str) -> Value {
    let qs = tool_input.get("questions").and_then(|q| q.as_array());
    let Some(qs) = qs else {
        return json!([]);
    };
    let mapped: Vec<Value> = qs
        .iter()
        .map(|q| {
            let question = q.get("question").and_then(|s| s.as_str()).unwrap_or("");
            let header = q.get("header").and_then(|s| s.as_str()).unwrap_or("");
            let multiple = q.get("multiSelect").and_then(|b| b.as_bool()).unwrap_or(false);
            let options: Vec<Value> = q
                .get("options")
                .and_then(|o| o.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|o| {
                            json!({
                                "label": o.get("label").and_then(|s| s.as_str()).unwrap_or(""),
                                "description": o.get("description").and_then(|s| s.as_str()).unwrap_or(""),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "question": question,
                "header": header,
                "options": options,
                "multiple": multiple,
                "custom": true,
            })
        })
        .collect();
    json!(mapped)
}

/// Format the user's answers into the hook deny-reason that delivers them to claude.
fn format_answers(tool_input: &Value, answers: &[Vec<String>]) -> String {
    let qs = tool_input
        .get("questions")
        .and_then(|q| q.as_array())
        .cloned()
        .unwrap_or_default();
    let mut lines = vec!["[USER ANSWER] The user answered your question(s):".to_string()];
    for (i, ans) in answers.iter().enumerate() {
        let q = qs
            .get(i)
            .and_then(|q| q.get("question"))
            .and_then(|s| s.as_str())
            .unwrap_or("(question)");
        lines.push(format!("  • {q} → {}", ans.join(", ")));
    }
    lines.push(
        "Treat these as the answers and continue; do NOT call AskUserQuestion again for the same question.".to_string(),
    );
    lines.join("\n")
}

async fn permission_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let reply = body
        .get("reply")
        .and_then(|r| r.as_str())
        .unwrap_or("once")
        .to_string();
    engine.resolve_pending(&id, super::PendingReply::Permission(reply));
    Json(json!({ "ok": true }))
}

async fn question_reply(
    State(engine): State<Engine>,
    Path(id): Path<String>,
    body: Json<Value>,
) -> Json<Value> {
    let answers: Vec<Vec<String>> = body
        .get("answers")
        .and_then(|a| serde_json::from_value(a.clone()).ok())
        .unwrap_or_default();
    engine.resolve_pending(&id, super::PendingReply::Question(answers));
    Json(json!({ "ok": true }))
}

async fn question_reject(State(engine): State<Engine>, Path(id): Path<String>) -> Json<Value> {
    engine.resolve_pending(&id, super::PendingReply::Reject);
    Json(json!({ "ok": true }))
}

async fn noop_obj() -> Json<Value> {
    Json(json!({}))
}

// ── SSE ─────────────────────────────────────────────────────────────

async fn event_stream(
    State(engine): State<Engine>,
    headers: HeaderMap,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let dir = dir_header(&headers);
    let mut rx = engine.subscribe();

    let stream = async_stream::stream! {
        // Initial connected event (opencode emits one; opman ignores unknowns).
        yield Ok::<_, Infallible>(
            Event::default().data(json!({ "type": "server.connected", "properties": {} }).to_string()),
        );
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if dir.is_empty() || ev.directory.is_empty() || ev.directory == dir {
                        yield Ok::<_, Infallible>(Event::default().data(ev.data));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[cfg(test)]
mod control_command_tests {
    use super::*;

    fn engine() -> Engine {
        Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
    }

    // `/agent <name>` must set the session's agent and NOT be forwarded to claude as a
    // prompt (which would no-op with "No response requested." and stall the session).
    // Names are translated to real claude agents: opencode's `build` has no claude
    // equivalent → cleared to the default agent; `plan` → claude's `Plan`.
    #[test]
    fn agent_command_translates_and_consumes() {
        let e = engine();
        let s = e.create_session("/d", "", "t");
        assert!(handle_control_command(&e, &s.id, "/agent build"));
        assert_eq!(e.get_session(&s.id).unwrap().agent, None); // build → default agent
        assert!(handle_control_command(&e, &s.id, "/agent plan"));
        assert_eq!(e.get_session(&s.id).unwrap().agent.as_deref(), Some("Plan"));
    }

    #[test]
    fn bare_agent_commands_are_swallowed_not_forwarded() {
        let e = engine();
        let s = e.create_session("/d", "", "t");
        assert!(handle_control_command(&e, &s.id, "/agent"));
        assert!(handle_control_command(&e, &s.id, "/agents")); // real claude CLI subcmd; useless as a bg prompt
        assert!(e.get_session(&s.id).unwrap().agent.is_none());
    }

    #[test]
    fn ordinary_slash_command_is_not_intercepted() {
        let e = engine();
        let s = e.create_session("/d", "", "t");
        // /compact is a genuine claude slash command — must fall through to a real turn.
        assert!(!handle_control_command(&e, &s.id, "/compact"));
    }
}
