//! Single-iteration drive tests for `tick_tailer`, exercised against a temp `HOME`
//! whose `~/.claude/projects/**` holds crafted claude transcripts. Covers the
//! skip-on-unchanged path, idle-expiry removal, subagent/background enrichment, and
//! title/message emission — all without spawning the polling task or a live `claude`.
use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use std::path::{Path, PathBuf};

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Point `HOME` at `home` for the duration of `f` (so `locate_*jsonl` glob the temp
/// transcript tree), restoring the prior value afterwards. Serialized via `ENV_LOCK`.
fn with_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
    let _g = env_guard();
    let prev = std::env::var_os("HOME");
    std::env::set_var("HOME", home);
    let out = f();
    match prev {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    out
}

fn engine() -> Arc<ClaudeEngine> {
    Arc::new(ClaudeEngine::new(
        None,
        crate::mcp_registry::RegistryHandle::default(),
    ))
}

fn set_uuid(e: &Arc<ClaudeEngine>, sid: &str, uuid: &str) {
    let mut g = e.reg.lock().unwrap();
    g.sessions.get_mut(sid).unwrap().claude_session_id = Some(uuid.to_string());
}

fn set_busy_flag(e: &Arc<ClaudeEngine>, sid: &str) {
    e.reg.lock().unwrap().sessions.get_mut(sid).unwrap().busy = true;
}

/// Write `<home>/.claude/projects/proj/<uuid>.jsonl` and return its path.
fn write_transcript(home: &Path, uuid: &str, content: &str) -> PathBuf {
    let projdir = home.join(".claude").join("projects").join("proj");
    std::fs::create_dir_all(&projdir).unwrap();
    let path = projdir.join(format!("{uuid}.jsonl"));
    std::fs::write(&path, content).unwrap();
    path
}

/// Write a subagent transcript under the per-turn `subagents/` directory.
fn write_subagent(home: &Path, aid: &str, content: &str) {
    let dir = home
        .join(".claude")
        .join("projects")
        .join("proj")
        .join("turn1")
        .join("subagents");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("agent-{aid}.jsonl")), content).unwrap();
}

/// A two-line main transcript that launches an async subagent with id `aid`.
fn main_with_task(aid: &str) -> String {
    format!(
        concat!(
            r#"{{"type":"assistant","timestamp":"2026-06-28T08:22:00.000Z","message":{{"id":"msg_1","model":"claude-haiku","content":[{{"type":"tool_use","id":"toolu_1","name":"Agent","input":{{"description":"Count files","prompt":"count"}}}}]}}}}"#,
            "\n",
            r#"{{"type":"user","timestamp":"2026-06-28T08:22:01.000Z","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":[{{"type":"text","text":"Async agent launched successfully.\nagentId: {aid} (internal ID - do not mention)"}}]}}]}}}}"#,
            "\n",
        ),
        aid = aid
    )
}

// ── early-return guards ─────────────────────────────────────────────

#[test]
fn tick_tailer_returns_false_when_session_gone() {
    let e = engine();
    let mut st = TailerState::default();
    assert!(!tick_tailer(&e, "nope", &mut st));
}

#[test]
fn tick_tailer_returns_true_without_uuid() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    let mut st = TailerState::default();
    // No claude_session_id yet → parked on the no-turn `continue` path.
    assert!(tick_tailer(&e, &s.id, &mut st));
}

#[test]
fn tick_tailer_resets_len_on_new_lineage_then_locate_miss() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-new");
    let home = tempfile::tempdir().unwrap(); // no projects tree → locate returns None
    let mut st = TailerState {
        last_uuid: Some("uuid-old".into()),
        last_len: 999,
        ..Default::default()
    };
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert_eq!(st.last_uuid.as_deref(), Some("uuid-new"));
    assert_eq!(st.last_len, 0); // lineage change forced a re-read; locate miss left it 0
}

// ── skip-on-unchanged path ──────────────────────────────────────────

#[test]
fn tick_tailer_skip_path_increments_idle() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-skip");
    let home = tempfile::tempdir().unwrap();
    let path = write_transcript(
        home.path(),
        "uuid-skip",
        "{\"type\":\"ai-title\",\"aiTitle\":\"x\"}\n",
    );
    let flen = std::fs::metadata(&path).unwrap().len();
    // Unchanged since last tick → skip the parse.
    let mut st = TailerState {
        idle_ticks: 5,
        last_len: flen,
        ..Default::default()
    };
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert_eq!(st.idle_ticks, 6);
}

#[test]
fn tick_tailer_skip_path_removes_when_idle_expired() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-skipexp");
    let home = tempfile::tempdir().unwrap();
    let path = write_transcript(home.path(), "uuid-skipexp", "{}\n");
    let flen = std::fs::metadata(&path).unwrap().len();
    e.tailers.lock().unwrap().insert(s.id.clone());
    // +1 → 601 > 600 → idle-expiry removal on the skip path.
    let mut st = TailerState {
        idle_ticks: 600,
        last_len: flen,
        ..Default::default()
    };
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(!cont);
    assert!(!e.tailers.lock().unwrap().contains(&s.id));
}

// ── full-parse tail: title + message emission ───────────────────────

#[test]
fn tick_tailer_emits_title_and_messages() {
    let content = concat!(
        r#"{"type":"ai-title","aiTitle":"My Session"}"#,
        "\n",
        r#"{"type":"user","timestamp":"2026-06-28T08:00:00.000Z","promptSource":"typed","message":{"role":"user","content":"hi"}}"#,
        "\n",
        r#"{"type":"assistant","timestamp":"2026-06-28T08:00:01.000Z","message":{"id":"msg_01","content":[{"type":"text","text":"hello"}]}}"#,
        "\n",
    );
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-basic");
    let home = tempfile::tempdir().unwrap();
    write_transcript(home.path(), "uuid-basic", content);
    // idle_ticks primed high; the new message resets it to 0.
    let mut st = TailerState {
        idle_ticks: 50,
        ..Default::default()
    };
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert_eq!(st.idle_ticks, 0);
    assert!(st.last_len > 0);
    assert_eq!(e.get_session(&s.id).unwrap().title, "My Session");
    assert!(st.emitted.contains_key("msg_01"));
}

#[test]
fn tick_tailer_full_parse_removes_when_idle_expired() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-idlerm");
    let home = tempfile::tempdir().unwrap();
    // Title-only transcript → no messages → any_new stays false.
    write_transcript(
        home.path(),
        "uuid-idlerm",
        "{\"type\":\"ai-title\",\"aiTitle\":\"t\"}\n",
    );
    e.tailers.lock().unwrap().insert(s.id.clone());
    // last_len 0 != cur_len → full parse, then idle-expiry removal at the bottom.
    let mut st = TailerState {
        idle_ticks: 600,
        last_len: 0,
        ..Default::default()
    };
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(!cont);
    assert!(!e.tailers.lock().unwrap().contains(&s.id));
}

#[test]
fn tick_tailer_busy_primes_idle_counter() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-busy");
    set_busy_flag(&e, &s.id);
    let home = tempfile::tempdir().unwrap();
    write_transcript(
        home.path(),
        "uuid-busy",
        "{\"type\":\"ai-title\",\"aiTitle\":\"t\"}\n",
    );
    let mut st = TailerState {
        idle_ticks: 42,
        last_len: 0,
        ..Default::default()
    };
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert_eq!(st.idle_ticks, 1); // busy: primed but non-exiting
}

// ── subagent enrichment ─────────────────────────────────────────────

#[test]
fn tick_tailer_subagent_running_marks_pending() {
    let aid = "agentrun01";
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-sub-run");
    let home = tempfile::tempdir().unwrap();
    write_transcript(home.path(), "uuid-sub-run", &main_with_task(aid));
    // Last line is an unfinished tool_use → subagent still running; file is fresh.
    write_subagent(
        home.path(),
        aid,
        r#"{"type":"assistant","timestamp":"2026-06-28T08:22:05.000Z","message":{"id":"sm1","content":[{"type":"tool_use","id":"tu","name":"Bash","input":{"command":"ls"}}]}}"#,
    );
    let mut st = TailerState::default();
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert!(st.has_pending_sub);
    let sub = e
        .get_session(aid)
        .expect("subagent child session registered");
    assert!(sub.is_subagent);
    assert!(sub.busy);
    assert!(e.subagent_pending(&s.id));
    assert!(st
        .emitted_sub
        .keys()
        .any(|k| k.starts_with(&format!("{aid}:"))));
}

#[test]
fn tick_tailer_subagent_completed_not_pending() {
    let aid = "agentdone01";
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-sub-done");
    let home = tempfile::tempdir().unwrap();
    write_transcript(home.path(), "uuid-sub-done", &main_with_task(aid));
    // Ends with an assistant text answer → subagent_completed → not running.
    write_subagent(
        home.path(),
        aid,
        r#"{"type":"assistant","timestamp":"2026-06-28T08:22:05.000Z","message":{"id":"sm1","content":[{"type":"text","text":"done counting"}]}}"#,
    );
    let mut st = TailerState::default();
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert!(!st.has_pending_sub);
    assert!(!e.subagent_pending(&s.id));
    assert!(!e.get_session(aid).unwrap().busy);
}

#[test]
fn tick_tailer_subagent_without_transcript_not_pending() {
    let aid = "agentghost01";
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-sub-ghost");
    let home = tempfile::tempdir().unwrap();
    write_transcript(home.path(), "uuid-sub-ghost", &main_with_task(aid));
    // No subagent file → locate miss → `continue` without marking pending.
    let mut st = TailerState::default();
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert!(!st.has_pending_sub);
    assert!(e.get_session(aid).is_some()); // child session still ensured
}

// ── background-task enrichment ──────────────────────────────────────

#[test]
fn tick_tailer_background_task_marks_pending_and_tails_output() {
    let e = engine();
    let s = e.create_session("/proj", "", "t");
    set_uuid(&e, &s.id, "uuid-bg");
    let home = tempfile::tempdir().unwrap();
    let out_file = home.path().join("bp1.output");
    std::fs::write(&out_file, "building...\n").unwrap();
    let main = format!(
        concat!(
            r#"{{"type":"assistant","timestamp":"2026-06-28T08:00:00.000Z","message":{{"id":"msg_1","content":[{{"type":"tool_use","id":"toolu_bg","name":"Bash","input":{{"command":"cargo build","description":"Build","run_in_background":true}}}}]}}}}"#,
            "\n",
            r#"{{"type":"user","timestamp":"2026-06-28T08:00:01.000Z","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_bg","content":"Command running in background with ID: bp1. Output is being written to: {out}. You will be notified when it completes."}}]}}}}"#,
            "\n",
        ),
        out = out_file.display()
    );
    write_transcript(home.path(), "uuid-bg", &main);
    let mut st = TailerState::default();
    let cont = with_home(home.path(), || tick_tailer(&e, &s.id, &mut st));
    assert!(cont);
    assert!(st.has_pending_bg); // running background task keeps the tail alive
}
