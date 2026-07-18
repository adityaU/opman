use super::*;
use crate::claude_p_engine::ClaudePEngine;
use std::sync::Arc;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

// ── pure helpers ─────────────────────────────────────────────────────

#[test]
fn hook_allow_deny_shapes() {
    let a = hook_allow();
    assert_eq!(a["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert_eq!(a["hookSpecificOutput"]["permissionDecision"], "allow");
    let d = hook_deny("because");
    assert_eq!(d["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(d["hookSpecificOutput"]["permissionDecisionReason"], "because");
}

#[test]
fn rand_request_id_shape() {
    let id = rand_request_id();
    assert!(id.starts_with("perm_"));
    assert_eq!(id.strip_prefix("perm_").unwrap().len(), 32);
    assert_ne!(rand_request_id(), rand_request_id());
}

#[test]
fn permission_patterns_collects_paths_and_command() {
    let input = json!({ "file_path": "/a", "path": "/b", "notebook_path": "/c", "command": "ls -l" });
    let p = permission_patterns(&input);
    assert_eq!(p, vec!["/a", "/b", "/c", "ls -l"]);
    assert!(permission_patterns(&json!({})).is_empty());
}

#[test]
fn build_questions_maps_and_defaults() {
    assert_eq!(build_questions(&json!({})), json!([]));
    let input = json!({ "questions": [
        { "question": "Q1", "header": "H1", "multiSelect": true, "options": [
            { "label": "L1", "description": "D1" }, { "label": "L2" }
        ]},
        { "question": "Q2" }
    ]});
    let out = build_questions(&input);
    assert_eq!(out[0]["question"], "Q1");
    assert_eq!(out[0]["header"], "H1");
    assert_eq!(out[0]["multiple"], true);
    assert_eq!(out[0]["custom"], true);
    assert_eq!(out[0]["options"][0]["label"], "L1");
    assert_eq!(out[0]["options"][1]["description"], "");
    // Missing options → empty list; missing multiSelect → false.
    assert_eq!(out[1]["options"], json!([]));
    assert_eq!(out[1]["multiple"], false);
}

#[test]
fn format_answers_renders_lines() {
    let input = json!({ "questions": [ { "question": "Pick a color" } ] });
    let answers = vec![vec!["red".to_string(), "blue".to_string()]];
    let s = format_answers(&input, &answers);
    assert!(s.contains("[USER ANSWER]"));
    assert!(s.contains("Pick a color → red, blue"));
    assert!(s.contains("do NOT ask again"));
    // Missing question index falls back to placeholder.
    let s2 = format_answers(&json!({}), &answers);
    assert!(s2.contains("(question)"));
}

// ── internal_ask: immediate (non-blocking) branches ──────────────────

#[tokio::test]
async fn internal_ask_no_session_allows() {
    let e = engine();
    let body = json!({ "session_id": "", "cwd": "", "tool_name": "Bash", "tool_input": {} });
    let Json(resp) = internal_ask(State(e), Json(body)).await;
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_bypass_mode_allows_gated() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "u1");
    // default mode is bypassPermissions.
    let body = json!({ "session_id": "u1", "cwd": "d", "tool_name": "Bash", "tool_input": {} });
    let Json(resp) = internal_ask(State(e), Json(body)).await;
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_non_gated_tool_allows() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_permission_mode(&s.id, "default");
    // No claude uuid → resolve by cwd (list_for_dir).
    let body = json!({ "session_id": "", "cwd": "d", "tool_name": "Read", "tool_input": {} });
    let Json(resp) = internal_ask(State(e), Json(body)).await;
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_accept_edits_allows_edit_tool() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "u2");
    e.set_permission_mode(&s.id, "acceptEdits");
    let body = json!({ "session_id": "u2", "cwd": "d", "tool_name": "Write", "tool_input": { "file_path": "/x" } });
    let Json(resp) = internal_ask(State(e), Json(body)).await;
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
}

#[tokio::test]
async fn internal_ask_plan_mode_denies_edit_tool() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "u3");
    e.set_permission_mode(&s.id, "plan");
    let body = json!({ "session_id": "u3", "cwd": "d", "tool_name": "Edit", "tool_input": {} });
    let Json(resp) = internal_ask(State(e), Json(body)).await;
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(resp["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap().contains("Plan mode"));
}

#[tokio::test]
async fn internal_ask_always_allowed_tool() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "u4");
    e.set_permission_mode(&s.id, "default");
    e.add_allowed_tool(&s.id, "Bash");
    let body = json!({ "session_id": "u4", "cwd": "d", "tool_name": "Bash", "tool_input": {} });
    let Json(resp) = internal_ask(State(e), Json(body)).await;
    assert_eq!(resp["hookSpecificOutput"]["permissionDecision"], "allow");
}

// ── reply endpoints ──────────────────────────────────────────────────

#[tokio::test]
async fn permission_reply_defaults_and_resolves() {
    let e = engine();
    // Unknown id still returns ok.
    let Json(r) = permission_reply(State(e.clone()), Path("nope".to_string()), Json(json!({}))).await;
    assert_eq!(r["ok"], true);
    // Registered id gets resolved.
    let _rx = e.register_pending("p1");
    let Json(r2) =
        permission_reply(State(e.clone()), Path("p1".to_string()), Json(json!({ "reply": "always" }))).await;
    assert_eq!(r2["ok"], true);
}

#[tokio::test]
async fn question_reply_parses_answers() {
    let e = engine();
    let _rx = e.register_pending("q1");
    let body = json!({ "answers": [["a", "b"], ["c"]] });
    let Json(r) = question_reply(State(e.clone()), Path("q1".to_string()), Json(body)).await;
    assert_eq!(r["ok"], true);
    // Malformed answers → default empty, still ok.
    let _rx2 = e.register_pending("q2");
    let Json(r2) =
        question_reply(State(e.clone()), Path("q2".to_string()), Json(json!({ "answers": "bad" }))).await;
    assert_eq!(r2["ok"], true);
}

#[tokio::test]
async fn question_reject_resolves() {
    let e = engine();
    let _rx = e.register_pending("q3");
    let Json(r) = question_reject(State(e.clone()), Path("q3".to_string())).await;
    assert_eq!(r["ok"], true);
}
