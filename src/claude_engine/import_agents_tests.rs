//! Coverage for `import_agents`' actual import loop (the branches beyond the skip
//! guards): titled-session import, untitled/no-transcript skips, and same-title
//! collapse — driven against a temp `HOME` transcript tree.
use super::*;
use crate::claude_engine::claude_cli::ENV_LOCK;
use std::path::Path;

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

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
    Arc::new(ClaudeEngine::new(None, (false, false, false, false)))
}

fn write_transcript(home: &Path, uuid: &str, content: &str) {
    let projdir = home.join(".claude").join("projects").join("proj");
    std::fs::create_dir_all(&projdir).unwrap();
    std::fs::write(projdir.join(format!("{uuid}.jsonl")), content).unwrap();
}

fn titled(title: &str) -> String {
    format!("{{\"type\":\"ai-title\",\"aiTitle\":\"{title}\"}}\n")
}

fn agent_at(
    session_id: &str,
    cwd: &str,
    started_at: u64,
    state: Option<&str>,
) -> claude_cli::AgentInfo {
    claude_cli::AgentInfo {
        id: "sh".into(),
        session_id: session_id.into(),
        cwd: cwd.into(),
        kind: String::new(),
        state: state.map(String::from),
        status: None,
        name: String::new(),
        started_at,
    }
}

#[test]
fn import_agents_imports_titled_session() {
    let home = tempfile::tempdir().unwrap();
    write_transcript(home.path(), "uuid-imp1", &titled("Imported One"));
    let e = engine();
    assert!(e.list_for_dir("/d").is_empty());
    with_home(home.path(), || {
        e.import_agents(
            "/d",
            vec![agent_at("uuid-imp1", "/d", 100, Some("working"))],
        );
    });
    let sessions = e.list_for_dir("/d");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Imported One");
    assert_eq!(sessions[0].claude_session_id.as_deref(), Some("uuid-imp1"));
    assert_eq!(sessions[0].lineage, vec!["uuid-imp1".to_string()]);
    assert_eq!(sessions[0].short_id.as_deref(), Some("sh"));
    assert!(sessions[0].busy); // state=working → is_busy()
}

#[test]
fn import_agents_skips_untitled_missing_and_collapses_duplicates() {
    let home = tempfile::tempdir().unwrap();
    // No ai-title line → read_ai_title None → skipped.
    write_transcript(
        home.path(),
        "uuid-notitle",
        "{\"type\":\"assistant\",\"message\":{\"id\":\"m\",\"content\":[]}}\n",
    );
    // Two transcripts sharing a title → collapse to the newest (dupB, started_at 30).
    write_transcript(home.path(), "uuid-dupA", &titled("Same Title"));
    write_transcript(home.path(), "uuid-dupB", &titled("Same Title"));
    // uuid-notrans has NO transcript file → locate miss → skipped.

    let e = engine();
    with_home(home.path(), || {
        e.import_agents(
            "/d",
            vec![
                agent_at("uuid-notitle", "/d", 10, None),
                agent_at("uuid-dupA", "/d", 20, None),
                agent_at("uuid-dupB", "/d", 30, None),
                agent_at("uuid-notrans", "/d", 40, None),
            ],
        );
    });
    let sessions = e.list_for_dir("/d");
    assert_eq!(
        sessions.len(),
        1,
        "only the newest same-titled session is imported"
    );
    assert_eq!(sessions[0].title, "Same Title");
    assert_eq!(sessions[0].claude_session_id.as_deref(), Some("uuid-dupB"));
}

#[test]
fn import_agents_second_call_is_idempotent() {
    let home = tempfile::tempdir().unwrap();
    write_transcript(home.path(), "uuid-once", &titled("Once"));
    let e = engine();
    with_home(home.path(), || {
        e.import_agents("/d", vec![agent_at("uuid-once", "/d", 5, None)]);
        // Second import sees the uuid already known → no duplicate row.
        e.import_agents("/d", vec![agent_at("uuid-once", "/d", 5, None)]);
    });
    assert_eq!(e.list_for_dir("/d").len(), 1);
}
