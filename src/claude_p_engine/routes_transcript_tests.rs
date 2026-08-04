//! Wave-2 coverage for `get_messages` / `get_todos` transcript parsing.
//!
//! Both locate on-disk JSONL via `claude_cli`, which reads `~/.claude/projects`.
//! We redirect `HOME` to a temp dir (under the shared `ENV_LOCK`) and plant
//! crafted transcripts so the parse+map paths execute deterministically.

use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use crate::claude_p_engine::ClaudePEngine;
use axum::extract::{Path, State};
use std::sync::Arc;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

/// Redirect HOME to a fresh temp dir; returns (guard, prev_home, tmpdir).
fn redirect_home() -> (
    std::sync::MutexGuard<'static, ()>,
    Option<String>,
    tempfile::TempDir,
) {
    let g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("HOME").ok();
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", tmp.path());
    (g, prev, tmp)
}

fn restore_home(prev: Option<String>) {
    match prev {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
}

/// Plant `~/.claude/projects/proj1/<uuid>.jsonl`.
fn plant_session_jsonl(home: &std::path::Path, uuid: &str, content: &str) {
    let proj = home.join(".claude/projects/proj1");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join(format!("{uuid}.jsonl")), content).unwrap();
}

/// Plant `~/.claude/projects/proj1/<turn>/subagents/agent-<id>.jsonl`.
fn plant_subagent_jsonl(home: &std::path::Path, agent_id: &str, content: &str) {
    let dir = home.join(".claude/projects/proj1/turn-abc/subagents");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("agent-{agent_id}.jsonl")), content).unwrap();
}

const USER_LINE: &str = r#"{"type":"user","promptSource":"typed","timestamp":"2026-06-28T08:00:00.000Z","message":{"role":"user","content":"do the thing"}}"#;

// ── get_messages ────────────────────────────────────────────────────

#[tokio::test]
async fn get_messages_parses_session_transcript() {
    let (_g, prev, tmp) = redirect_home();
    let e = engine();
    let s = e.create_session("d", "", "A");
    let uuid = format!("u-{:x}", rand::random::<u64>());
    e.set_claude_uuid(&s.id, &uuid);
    plant_session_jsonl(tmp.path(), &uuid, &format!("{USER_LINE}\n"));

    let Json(v) = get_messages(State(e), Path(s.id.clone())).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    // Parsed user bubble id is `msg_user_<session_id>_1`.
    let id = arr[0]["info"]["id"].as_str().unwrap();
    assert!(id.starts_with("msg_user_"), "unexpected id: {id}");
    restore_home(prev);
}

#[tokio::test]
async fn get_messages_subagent_child_branch() {
    let (_g, prev, tmp) = redirect_home();
    let e = engine();
    let parent = e.create_session("d", "", "P");
    e.ensure_subagent_session(&parent.id, "sub-1", "", "d");
    plant_subagent_jsonl(tmp.path(), "sub-1", &format!("{USER_LINE}\n"));

    let Json(v) = get_messages(State(e), Path("sub-1".to_string())).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
    restore_home(prev);
}

#[tokio::test]
async fn get_messages_subagent_child_missing_file_is_empty() {
    let (_g, prev, _tmp) = redirect_home();
    let e = engine();
    let parent = e.create_session("d", "", "P");
    e.ensure_subagent_session(&parent.id, "sub-2", "", "d");
    let Json(v) = get_messages(State(e), Path("sub-2".to_string())).await;
    assert!(v.as_array().unwrap().is_empty());
    restore_home(prev);
}

#[tokio::test]
async fn get_messages_backfill_unknown_id_via_subagent_file() {
    let (_g, prev, tmp) = redirect_home();
    let e = engine();
    // Id not registered as a session, but a subagent transcript exists on disk.
    plant_subagent_jsonl(tmp.path(), "orphan-9", &format!("{USER_LINE}\n"));
    let Json(v) = get_messages(State(e), Path("orphan-9".to_string())).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
    restore_home(prev);
}

#[tokio::test]
async fn get_messages_unknown_and_no_uuid_are_empty() {
    let (_g, prev, _tmp) = redirect_home();
    let e = engine();
    // Unknown id, no subagent file → empty.
    let Json(v1) = get_messages(State(e.clone()), Path("nope".to_string())).await;
    assert!(v1.as_array().unwrap().is_empty());
    // Session without a claude uuid → empty.
    let s = e.create_session("d", "", "A");
    let Json(v2) = get_messages(State(e.clone()), Path(s.id.clone())).await;
    assert!(v2.as_array().unwrap().is_empty());
    // Session with a uuid but no file on disk → empty.
    e.set_claude_uuid(&s.id, "missing-uuid");
    let Json(v3) = get_messages(State(e), Path(s.id)).await;
    assert!(v3.as_array().unwrap().is_empty());
    restore_home(prev);
}

// ── get_todos ───────────────────────────────────────────────────────

#[tokio::test]
async fn get_todos_extracts_todowrite_items() {
    let (_g, prev, tmp) = redirect_home();
    let e = engine();
    let s = e.create_session("d", "", "A");
    let uuid = format!("u-{:x}", rand::random::<u64>());
    e.set_claude_uuid(&s.id, &uuid);
    let line = r#"{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"TodoWrite","input":{"todos":[{"content":"first","status":"pending"},{"content":"second","status":"completed"}]}}]}}"#;
    plant_session_jsonl(tmp.path(), &uuid, &format!("{line}\n"));

    let Json(v) = get_todos(State(e), Path(s.id)).await;
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["content"], "first");
    assert_eq!(arr[1]["status"], "completed");
    restore_home(prev);
}

#[tokio::test]
async fn get_todos_no_uuid_is_empty() {
    let (_g, prev, _tmp) = redirect_home();
    let e = engine();
    let s = e.create_session("d", "", "A");
    let Json(v) = get_todos(State(e), Path(s.id)).await;
    assert!(v.as_array().unwrap().is_empty());
    restore_home(prev);
}

#[tokio::test]
async fn get_todos_uuid_without_file_is_empty() {
    let (_g, prev, _tmp) = redirect_home();
    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, "no-such-file");
    let Json(v) = get_todos(State(e), Path(s.id)).await;
    assert!(v.as_array().unwrap().is_empty());
    restore_home(prev);
}
