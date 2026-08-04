//! Wave-2 coverage for the `claude -p` process driver: the extracted
//! `reparse_emit_from_path` emit body, the `send` stdin-write success/closed
//! branches, and a `spawn` success path driven by a fake `claude` binary.

use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;

fn engine() -> Arc<ClaudePEngine> {
    Arc::new(ClaudePEngine::new(None, (false, false, false, false)))
}

fn drain(
    rx: &mut tokio::sync::broadcast::Receiver<crate::claude_engine::EngineEvent>,
) -> Vec<String> {
    let mut out = vec![];
    while let Ok(ev) = rx.try_recv() {
        out.push(ev.data);
    }
    out
}

/// Write a transcript file and return its path (kept alive by the returned dir).
fn write_transcript(lines: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    for l in lines {
        writeln!(f, "{l}").unwrap();
    }
    (dir, path)
}

// ── reparse_emit_from_path ──────────────────────────────────────────

#[tokio::test]
async fn reparse_from_path_emits_messages_and_title_and_dedupes() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    let (_tmp, path) = write_transcript(&[
        r#"{"type":"ai-title","aiTitle":"My Title"}"#,
        r#"{"type":"user","promptSource":"typed","timestamp":"2026-06-28T08:00:00.000Z","message":{"role":"user","content":"hello world"}}"#,
    ]);

    reparse_emit_from_path(&e, &s.id, "d", path.clone()).await;
    let events = drain(&mut rx);
    // At least the message.updated + message.part.updated for the user bubble.
    assert!(events.iter().any(|d| d.contains("message.updated")));
    assert!(events.iter().any(|d| d.contains("message.part.updated")));
    assert!(events.iter().any(|d| d.contains("hello world")));
    // Title propagated to the session (non-manual set).
    assert_eq!(e.get_session(&s.id).unwrap().title, "My Title");

    // Second pass over identical content emits nothing (should_emit gate).
    let mut rx2 = e.subscribe();
    reparse_emit_from_path(&e, &s.id, "d", path).await;
    let events2 = drain(&mut rx2);
    assert!(
        events2.iter().all(|d| !d.contains("message.updated")),
        "unchanged messages must not re-emit"
    );
}

#[tokio::test]
async fn reparse_from_path_registers_subagents() {
    let e = engine();
    let s = e.create_session("dir-x", "", "A");
    // An Agent tool_use whose launch-ack tool_result carries an `agentId` maps to a
    // `task` part and registers the child in `subagent_ids`, driving the
    // `ensure_subagent_session` nesting loop.
    let aid = "a1834b2decb148144";
    let (_tmp, path) = write_transcript(&[
        r#"{"type":"assistant","timestamp":"2026-06-28T08:22:00.000Z","message":{"id":"msg_1","content":[{"type":"tool_use","id":"toolu_1","name":"Agent","input":{"description":"Count files","prompt":"count"}}]}}"#,
        r#"{"type":"user","timestamp":"2026-06-28T08:22:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"Async agent launched successfully.\nagentId: a1834b2decb148144 (internal ID - do not mention)"}]}]}}"#,
    ]);
    reparse_emit_from_path(&e, &s.id, "dir-x", path).await;
    let child = e
        .get_session(aid)
        .expect("subagent child session registered");
    assert!(child.is_subagent);
    assert_eq!(child.parent_id, s.id);
    assert_eq!(child.directory, "dir-x");
}

#[tokio::test]
async fn reparse_from_path_missing_file_is_noop() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    let mut rx = e.subscribe();
    reparse_emit_from_path(
        &e,
        &s.id,
        "d",
        std::path::PathBuf::from("/nonexistent/x.jsonl"),
    )
    .await;
    assert!(drain(&mut rx).is_empty());
}

#[tokio::test]
async fn reparse_emit_locates_via_home_redirect() {
    // Drive the full reparse_emit (including locate_jsonl) by planting a
    // transcript under a redirected HOME.
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("HOME").ok();
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", tmp.path());
    let uuid = format!("uuid-{:x}", rand::random::<u64>());
    let proj = tmp.path().join(".claude/projects/proj1");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(
        proj.join(format!("{uuid}.jsonl")),
        "{\"type\":\"user\",\"promptSource\":\"typed\",\"message\":{\"role\":\"user\",\"content\":\"hi there\"}}\n",
    )
    .unwrap();

    let e = engine();
    let s = e.create_session("d", "", "A");
    e.set_claude_uuid(&s.id, &uuid);
    let mut rx = e.subscribe();
    reparse_emit(&e, &s.id).await;
    let events = drain(&mut rx);
    assert!(
        events.iter().any(|d| d.contains("hi there")),
        "located + emitted transcript"
    );

    match prev {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
}

// ── send: stdin-write branches ──────────────────────────────────────

async fn spawn_cat() -> Proc {
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    Proc { stdin, child }
}

#[tokio::test]
async fn send_write_success_sets_busy() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    // Pre-seed a live `cat` process so send() takes the write-success branch.
    e.procs
        .0
        .lock()
        .await
        .insert(s.id.clone(), spawn_cat().await);
    send(e.clone(), s.id.clone(), "steer me".to_string()).await;
    assert!(
        e.get_session(&s.id).unwrap().busy,
        "successful stdin write marks busy"
    );
    assert!(
        e.procs.0.lock().await.contains_key(&s.id),
        "process retained"
    );
    // Clean up the live child.
    abort(e.clone(), &s.id).await;
}

#[tokio::test]
async fn send_write_failure_drops_process() {
    let e = engine();
    let s = e.create_session("d", "", "A");
    // Spawn `true`, wait for it to exit, so its stdin pipe is broken → write fails.
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("exit 0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let _ = child.wait().await;
    e.procs
        .0
        .lock()
        .await
        .insert(s.id.clone(), Proc { stdin, child });
    e.set_busy(&s.id, true);
    send(e.clone(), s.id.clone(), "will fail".to_string()).await;
    // Broken pipe → process removed and busy cleared.
    assert!(!e.procs.0.lock().await.contains_key(&s.id));
    assert!(!e.get_session(&s.id).unwrap().busy);
}

// ── spawn success via a fake `claude` binary ────────────────────────

#[tokio::test]
async fn send_spawns_via_fake_claude_binary() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();

    // A fake `claude` that reads stdin and emits stream-json init + result.
    let bindir = tempfile::tempdir().unwrap();
    let script = bindir.path().join("fake-claude");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         printf '%s\\n' '{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fake-uuid\"}'\n\
         # drain one line of stdin then finish the turn\n\
         head -n 1 >/dev/null 2>&1\n\
         printf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\"}'\n\
         sleep 0.2\n",
    )
    .unwrap();
    let mut perm = std::fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&script, perm).unwrap();
    std::env::set_var("OPMAN_CLAUDE_BIN", &script);

    // A real, existing cwd is required (Command::current_dir).
    let cwd = tempfile::tempdir().unwrap();
    let e = engine();
    let s = e.create_session(&cwd.path().to_string_lossy(), "", "A");

    send(e.clone(), s.id.clone(), "hello".to_string()).await;
    // Spawn succeeded → a process is registered and the session is busy.
    assert!(
        e.procs.0.lock().await.contains_key(&s.id),
        "fake claude spawned"
    );
    assert!(e.get_session(&s.id).unwrap().busy);

    // Give the reader a beat to record the init uuid, then tear down.
    for _ in 0..40 {
        if e.get_session(&s.id).and_then(|x| x.claude_uuid).is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    abort(e.clone(), &s.id).await;

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
}
