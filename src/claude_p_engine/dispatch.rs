//! Turn dispatch for the `claude -p` engine: extract prompt text, intercept runtime
//! control commands (`/agent`, `/permission-mode`), else push the message to the
//! session's running process.

use serde_json::{json, Value};

use super::process;
use super::routes::Engine;

const PERMISSION_MODES: &[&str] =
    &["default", "acceptEdits", "auto", "bypassPermissions", "dontAsk", "plan"];

/// Pull the prompt text out of a send body (`parts[]` of type text, or `text`/`prompt`).
pub(super) fn extract_text(body: &Value) -> String {
    if let Some(parts) = body.get("parts").and_then(|p| p.as_array()) {
        let joined = parts
            .iter()
            .filter_map(|p| {
                let t = p.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                (t == "text").then(|| p.get("text").and_then(|t| t.as_str())).flatten()
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !joined.is_empty() {
            return joined;
        }
    }
    body.get("text")
        .and_then(|t| t.as_str())
        .or_else(|| body.get("prompt").and_then(|t| t.as_str()))
        .unwrap_or("")
        .to_string()
}

/// Apply a runtime control command (`/agent`, `/permission-mode`); true = consumed.
fn handle_control_command(engine: &Engine, session_id: &str, text: &str) -> bool {
    let t = text.trim();
    if t == "/agent" || t == "/agents" {
        return true;
    }
    if let Some(name) = t.strip_prefix("/agent ") {
        let name = name.trim();
        if !name.is_empty() {
            engine.set_agent(session_id, name);
            if let Some(s) = engine.get_session(session_id) {
                engine.emit(
                    &s.directory,
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
        match PERMISSION_MODES.iter().find(|m| m.eq_ignore_ascii_case(mode)).copied() {
            Some(m) => engine.set_permission_mode(session_id, m),
            None => {
                if let Some(s) = engine.get_session(session_id) {
                    engine.emit(
                        &s.directory,
                        "tui.toast.show",
                        json!({ "message": format!("Unknown permission mode '{mode}'"), "variant": "error" }),
                    );
                }
            }
        }
        return true;
    }
    false
}

/// Dispatch a user turn: control command, or push to the running `claude -p` process.
pub(super) fn dispatch_turn(engine: Engine, session_id: String, text: String) {
    if text.trim().is_empty() {
        return;
    }
    if handle_control_command(&engine, &session_id, &text) {
        return;
    }
    tokio::spawn(process::send(engine, session_id, text));
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod dispatch_tests;
