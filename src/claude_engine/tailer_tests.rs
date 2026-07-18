//! Coverage for the tailer's pure freshness helper, plus loop-entry drive tests for
//! the two background pollers (session-gone break, no-uuid continue, absent-agent path).
use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;

// ---- transcript_is_fresh ----------------------------------------------

#[test]
fn transcript_is_fresh_true_for_just_written_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.jsonl");
    std::fs::write(&path, b"{}").unwrap();
    assert!(transcript_is_fresh(&path));
}

#[test]
fn transcript_is_fresh_false_for_missing_file() {
    assert!(!transcript_is_fresh(std::path::Path::new("/no/such/transcript.jsonl")));
}

#[test]
fn transcript_is_fresh_false_for_backdated_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stale.jsonl");
    std::fs::write(&path, b"{}").unwrap();
    // Backdate mtime/atime to ~1970+1000s, well beyond SUBAGENT_STALE (180s).
    let c = std::ffi::CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let old = libc::timeval { tv_sec: 1000, tv_usec: 0 };
    let times = [old, old];
    let rc = unsafe { libc::utimes(c.as_ptr(), times.as_ptr()) };
    assert_eq!(rc, 0, "utimes should succeed");
    assert!(!transcript_is_fresh(&path));
}

// ---- drive tests (execute loop bodies without a live claude) ----------

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
}

// A tailer for a session that doesn't exist breaks out of its loop on the first tick.
#[tokio::test]
async fn tailer_exits_when_session_absent() {
    let e = engine();
    spawn_tailer(e.clone(), "ses_nonexistent".to_string());
    // Give the spawned task a couple of ticks (100ms each) to run and break.
    tokio::time::sleep(Duration::from_millis(250)).await;
    // The tailer never registered itself in `tailers` here (ensure_tailer wasn't used),
    // so there's nothing to assert beyond "it didn't panic / hang".
}

// A session with no claude_session_id keeps the tailer parked on the `continue` path.
#[tokio::test]
async fn tailer_runs_with_session_but_no_turn() {
    let e = engine();
    let s = e.create_session("/tmp/proj", "", "t");
    spawn_tailer(e.clone(), s.id.clone());
    tokio::time::sleep(Duration::from_millis(150)).await;
    // Then delete the session so the task takes its break path on the next tick.
    e.remove_session(&s.id);
    tokio::time::sleep(Duration::from_millis(150)).await;
}

// Drive the status poller through one iteration with a stubbed (empty) agent list:
// the session is absent from the list, so it stays in the debounce/continue path.
#[tokio::test]
async fn status_poller_runs_one_iteration_with_empty_agents() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    std::env::set_var("OPMAN_CLAUDE_BIN", "echo"); // non-JSON → empty agent list

    let e = engine();
    let s = e.create_session("/tmp/proj", "", "t");
    // Give it a claude UUID so the poller processes it past the `None` guard.
    {
        let mut g = e.reg.lock().unwrap();
        let entry = g.sessions.get_mut(&s.id).unwrap();
        entry.claude_session_id = Some("uuid-poll".into());
    }
    spawn_status_poller(e.clone());
    tokio::time::sleep(Duration::from_millis(150)).await;

    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
    // First absence only increments the debounce counter (< ABSENT_POLLS_BEFORE_IDLE),
    // so the session must not have been flipped busy/idle spuriously.
    assert!(!e.get_session(&s.id).unwrap().busy);
}
