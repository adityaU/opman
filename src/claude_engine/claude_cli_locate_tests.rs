//! Coverage for the `is_file()` success arms of `locate_jsonl`/`locate_subagent_jsonl`
//! (via a temp `HOME` transcript tree), plus the `introspect` init-parse path and the
//! `bg_start`/`bg_resume` success paths driven by a fake `claude` binary on `OPMAN_CLAUDE_BIN`.
use super::*;
use std::path::Path;

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run `f` with `HOME=home` (glob root for the locate helpers), restoring afterwards.
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

/// Run `f` with `OPMAN_CLAUDE_BIN=bin`, restoring afterwards.
fn with_bin<T>(bin: &str, f: impl FnOnce() -> T) -> T {
    let _g = env_guard();
    let prev = std::env::var("OPMAN_CLAUDE_BIN").ok();
    std::env::set_var("OPMAN_CLAUDE_BIN", bin);
    let out = f();
    match prev {
        Some(v) => std::env::set_var("OPMAN_CLAUDE_BIN", v),
        None => std::env::remove_var("OPMAN_CLAUDE_BIN"),
    }
    out
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

// ── locate_jsonl / locate_subagent_jsonl success arms ───────────────

#[test]
fn locate_jsonl_finds_file_and_ignores_missing() {
    let home = tempfile::tempdir().unwrap();
    let projects = home.path().join(".claude").join("projects");
    let projdir = projects.join("encoded-cwd");
    std::fs::create_dir_all(&projdir).unwrap();
    std::fs::write(projdir.join("uuid-abc.jsonl"), b"{}").unwrap();
    // A stray file directly under projects/ must not confuse the glob.
    std::fs::write(projects.join("stray.txt"), b"x").unwrap();

    let (found, missing) = with_home(home.path(), || {
        (locate_jsonl("uuid-abc"), locate_jsonl("uuid-none"))
    });
    assert!(found.as_ref().unwrap().ends_with("uuid-abc.jsonl"));
    assert!(missing.is_none());
}

#[test]
fn locate_subagent_jsonl_finds_nested_file_and_skips_non_dirs() {
    let home = tempfile::tempdir().unwrap();
    let base = home
        .path()
        .join(".claude")
        .join("projects")
        .join("proj")
        .join("turnX")
        .join("subagents");
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("agent-aid99.jsonl"), b"{}").unwrap();
    // A loose file directly under projects/ exercises the `!pdir.is_dir()` skip.
    std::fs::write(home.path().join(".claude").join("projects").join("loosefile"), b"x").unwrap();

    let (found, missing) = with_home(home.path(), || {
        (locate_subagent_jsonl("aid99"), locate_subagent_jsonl("aid-absent"))
    });
    assert!(found.as_ref().unwrap().ends_with("agent-aid99.jsonl"));
    assert!(missing.is_none());
}

// ── introspect init-parse path ──────────────────────────────────────

#[test]
fn introspect_parses_init_event() {
    let (bin, _dir) = make_script(concat!(
        "echo 'garbage-not-json'\n",
        "echo '{\"type\":\"system\",\"subtype\":\"other\"}'\n",
        "echo '{\"type\":\"system\",\"subtype\":\"init\",\"slash_commands\":[\"compact\",\"clear\"],\"agents\":[\"claude\",\"Plan\"]}'",
    ));
    let info = with_bin(&bin, || introspect("/tmp"));
    assert_eq!(info.commands, vec!["compact".to_string(), "clear".to_string()]);
    assert_eq!(info.agents, vec!["claude".to_string(), "Plan".to_string()]);
}

// ── bg_start / bg_resume success (fake claude) ──────────────────────

/// A fake `claude`: `--bg` prints the backgrounded ack; `agents` prints a JSON list
/// whose short id matches, so `run_bg` resolves the full session UUID immediately.
fn fake_claude() -> (String, tempfile::TempDir) {
    make_script(concat!(
        "case \"$1\" in\n",
        "  --bg) echo 'backgrounded · sid123' ;;\n",
        "  agents) echo '[{\"id\":\"sid123\",\"sessionId\":\"uuid-xyz\",\"cwd\":\"/d\",\"state\":\"working\"}]' ;;\n",
        "esac",
    ))
}

#[test]
fn bg_start_success_resolves_uuid() {
    let (bin, _dir) = fake_claude();
    let opts = TurnOpts::default();
    let (sid, uuid) = with_bin(&bin, || bg_start("/tmp", &opts, "hello")).unwrap();
    assert_eq!(sid, "sid123");
    assert_eq!(uuid, "uuid-xyz");
}

#[test]
fn bg_resume_success_resolves_uuid() {
    let (bin, _dir) = fake_claude();
    let opts = TurnOpts::default();
    let (sid, uuid) = with_bin(&bin, || bg_resume("/tmp", "old-uuid", &opts, "more")).unwrap();
    assert_eq!(sid, "sid123");
    assert_eq!(uuid, "uuid-xyz");
}

// ── version success ─────────────────────────────────────────────────

#[test]
fn version_returns_trimmed_output() {
    // A fake binary that prints a padded version string; version() trims it.
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("fakever.sh");
    std::fs::write(&bin, "#!/bin/sh\nprintf '  claude 9.9.9  \\n'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(with_bin(bin.to_str().unwrap(), version), "claude 9.9.9");
}
