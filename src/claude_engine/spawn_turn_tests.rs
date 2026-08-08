//! Drive `spawn_turn`'s background task to completion against a fake `claude` binary,
//! covering the async spawn arms the synchronous guard test can't reach: the success
//! path (bg_start/bg_resume → `record_turn`) and the failure path (unparseable output
//! → `emit_system` error + `set_busy(false)`).
use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use std::time::Duration;

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ))
}

/// Write an executable `/bin/sh` script; return (path, tempdir keeping it alive).
fn make_script(body: &str) -> (String, tempfile::TempDir) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fake-claude");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    (path.to_string_lossy().into_owned(), dir)
}

/// A fake `claude`: `--bg` prints the backgrounded ack; `agents` prints a matching
/// JSON list so `run_bg` resolves the full session UUID on the first poll.
fn fake_claude() -> (String, tempfile::TempDir) {
    make_script(concat!(
        "case \"$1\" in\n",
        "  --bg) echo 'backgrounded · sid123' ;;\n",
        "  agents) echo '[{\"id\":\"sid123\",\"sessionId\":\"uuid-xyz\",\"cwd\":\"/d\",\"state\":\"working\"}]' ;;\n",
        "esac",
    ))
}

/// Poll until the session's dispatch guard clears (background task finished) or timeout.
async fn wait_dispatch_clear(e: &Arc<ClaudeEngine>, sid: &str) {
    for _ in 0..200 {
        if !e.is_dispatching(sid) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("spawn_turn background task did not finish");
}

#[tokio::test]
async fn spawn_turn_success_records_turn() {
    let _g = env_guard();
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    let (bin, _dir) = fake_claude();
    std::env::set_var("OPMAN_CLAUDE_BIN", &bin);

    let e = engine();
    let s = e.create_session("/tmp", "", "t");
    e.spawn_turn(s.id.clone(), "do work".into());
    // Synchronously guarded up front.
    assert!(e.is_dispatching(&s.id));
    assert!(e.get_session(&s.id).unwrap().busy);
    wait_dispatch_clear(&e, &s.id).await;

    let entry = e.get_session(&s.id).unwrap();
    assert_eq!(entry.short_id.as_deref(), Some("sid123"));
    assert_eq!(entry.claude_session_id.as_deref(), Some("uuid-xyz"));
    assert!(entry.busy); // record_turn marks busy
    assert!(!e.is_dispatching(&s.id)); // dispatch guard cleared

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}

#[tokio::test]
async fn spawn_turn_resume_path_records_turn() {
    let _g = env_guard();
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    let (bin, _dir) = fake_claude();
    std::env::set_var("OPMAN_CLAUDE_BIN", &bin);

    let e = engine();
    let s = e.create_session("/tmp", "", "t");
    // A prior claude uuid routes spawn_turn through the `bg_resume` arm.
    e.reg
        .lock()
        .unwrap()
        .sessions
        .get_mut(&s.id)
        .unwrap()
        .claude_session_id = Some("old-uuid".into());
    e.spawn_turn(s.id.clone(), "keep going".into());
    wait_dispatch_clear(&e, &s.id).await;

    let entry = e.get_session(&s.id).unwrap();
    assert_eq!(entry.short_id.as_deref(), Some("sid123"));
    assert_eq!(entry.claude_session_id.as_deref(), Some("uuid-xyz"));

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}

#[tokio::test]
async fn spawn_turn_failure_emits_error_and_clears_busy() {
    let _g = env_guard();
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    // `echo` prints the args (no "backgrounded · <id>" line) → bg_start errors.
    std::env::set_var("OPMAN_CLAUDE_BIN", "echo");

    let e = engine();
    let s = e.create_session("/tmp", "", "t");
    let mut rx = e.subscribe();
    e.spawn_turn(s.id.clone(), "will fail".into());
    wait_dispatch_clear(&e, &s.id).await;

    // Failure path resets busy and surfaces a system error bubble.
    assert!(!e.get_session(&s.id).unwrap().busy);
    // Drain events until the error message.updated (system bubble) arrives.
    let mut saw_error = false;
    for _ in 0..20 {
        match rx.try_recv() {
            Ok(ev) => {
                let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
                if v["type"] == "message.updated" && v["properties"]["info"]["level"] == "error" {
                    saw_error = true;
                    break;
                }
            }
            Err(_) => break,
        }
    }
    assert!(saw_error, "expected a system error bubble on spawn failure");

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}

#[tokio::test]
async fn failed_queued_turn_is_requeued_for_retry() {
    let e = engine();
    let s = e.create_session("/tmp", "", "t");
    e.set_busy(&s.id, true);
    e.enqueue_prompt(&s.id, "follow-up".into());
    let text = e.take_pending(&s.id).expect("queued prompt");

    e.finish_turn_failure(&s.id, text, Some(0));

    assert!(!e.get_session(&s.id).unwrap().busy);
    assert!(!e.is_dispatching(&s.id));
    assert_eq!(e.pending_list(&s.id), vec!["follow-up"]);
}
