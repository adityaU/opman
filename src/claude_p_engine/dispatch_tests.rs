use super::*;
use crate::claude_p_engine::ClaudePEngine;
use std::sync::Arc;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

#[test]
fn extract_text_from_parts() {
    let body = json!({ "parts": [
        { "type": "text", "text": "hello" },
        { "type": "text", "text": "world" },
    ]});
    assert_eq!(extract_text(&body), "hello\nworld");
}

#[test]
fn extract_text_default_type_is_text() {
    // A part without an explicit type defaults to "text".
    let body = json!({ "parts": [ { "text": "implicit" } ] });
    assert_eq!(extract_text(&body), "implicit");
}

#[test]
fn extract_text_skips_non_text_parts() {
    let body = json!({ "parts": [
        { "type": "image", "text": "ignored" },
        { "type": "text", "text": "kept" },
    ]});
    assert_eq!(extract_text(&body), "kept");
}

#[test]
fn extract_text_parts_empty_falls_back_to_text_field() {
    // All parts non-text → joined empty → fall back.
    let body = json!({ "parts": [ { "type": "image", "url": "x" } ], "text": "fallback" });
    assert_eq!(extract_text(&body), "fallback");
}

#[test]
fn extract_text_from_text_and_prompt_fields() {
    assert_eq!(extract_text(&json!({ "text": "t" })), "t");
    assert_eq!(extract_text(&json!({ "prompt": "p" })), "p");
    assert_eq!(extract_text(&json!({})), "");
}

#[test]
fn control_command_agent_listing() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    assert!(handle_control_command(&e, &s.id, "/agent"));
    assert!(handle_control_command(&e, &s.id, "  /agents  "));
}

#[test]
fn control_command_set_agent() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    assert!(handle_control_command(&e, &s.id, "/agent Researcher"));
    assert_eq!(e.get_session(&s.id).unwrap().agent.as_deref(), Some("Researcher"));
    // "/agent " with empty name is consumed but sets nothing.
    assert!(handle_control_command(&e, &s.id, "/agent "));
}

#[test]
fn control_command_set_agent_missing_session() {
    let e = engine();
    // No session → consumed, toast skipped (get_session None).
    assert!(handle_control_command(&e, "nope", "/agent Foo"));
}

#[test]
fn control_command_permission_modes() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    assert!(handle_control_command(&e, &s.id, "/permission-mode plan"));
    assert_eq!(e.get_session(&s.id).unwrap().permission_mode.as_deref(), Some("plan"));
    assert!(handle_control_command(&e, &s.id, "/perm-mode acceptEdits"));
    assert_eq!(e.get_session(&s.id).unwrap().permission_mode.as_deref(), Some("acceptEdits"));
    // Case-insensitive match against the canonical spelling.
    assert!(handle_control_command(&e, &s.id, "/perm BYPASSPERMISSIONS"));
    assert_eq!(e.get_session(&s.id).unwrap().permission_mode.as_deref(), Some("bypassPermissions"));
}

#[test]
fn control_command_unknown_permission_mode_emits_error() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    assert!(handle_control_command(&e, &s.id, "/permission-mode bogus"));
    // Mode unchanged, error toast emitted.
    assert!(e.get_session(&s.id).unwrap().permission_mode.is_none());
    let ev = rx.recv().now_or_never().unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
    assert_eq!(v["type"], "tui.toast.show");
    assert_eq!(v["properties"]["variant"], "error");
}

#[test]
fn control_command_unknown_permission_mode_missing_session() {
    let e = engine();
    assert!(handle_control_command(&e, "nope", "/perm bogus"));
}

#[test]
fn control_command_not_a_command() {
    let e = engine();
    assert!(!handle_control_command(&e, "id", "just a normal message"));
    assert!(!handle_control_command(&e, "id", "/unknown-slash"));
}

#[tokio::test]
async fn dispatch_turn_empty_is_noop() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    dispatch_turn(e.clone(), s.id.clone(), "   ".to_string());
    assert!(!e.get_session(&s.id).unwrap().busy);
}

#[tokio::test]
async fn dispatch_turn_control_command_consumed() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    dispatch_turn(e.clone(), s.id.clone(), "/permission-mode plan".to_string());
    assert_eq!(e.get_session(&s.id).unwrap().permission_mode.as_deref(), Some("plan"));
}

#[tokio::test]
async fn dispatch_turn_plain_text_spawns_send() {
    let e = engine();
    // Nonexistent directory → the child spawn fails fast (chdir error), no real claude.
    let n: u128 = rand::random();
    let s = e.create_session(&format!("/nonexistent/opman_{n:032x}"), "", "A");
    dispatch_turn(e.clone(), s.id.clone(), "hello there".to_string());
    // Let the spawned task run and fail.
    for _ in 0..50 {
        tokio::task::yield_now().await;
        if !e.get_session(&s.id).unwrap().busy {
            break;
        }
    }
    assert!(!e.get_session(&s.id).unwrap().busy);
}

use futures::FutureExt;
