//! Coverage for claude_cli pure builders/parsers + binary-absent error branches.
use super::*;

/// Lock the shared env mutex, recovering from poisoning.
fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run `f` with `OPMAN_CLAUDE_BIN=bin`, restoring the prior value afterwards.
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

fn args_of(cmd: &std::process::Command) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

// ---- parse_short_id ----------------------------------------------------

#[test]
fn parse_short_id_variants() {
    assert_eq!(
        parse_short_id("backgrounded · ae842e84"),
        Some("ae842e84".into())
    );
    assert_eq!(
        parse_short_id("noise\nbackgrounded · deadbeef\nmore"),
        Some("deadbeef".into())
    );
    // No marker at all.
    assert_eq!(parse_short_id("nothing to see"), None);
    // The word alone (no id token) → None.
    assert_eq!(parse_short_id("backgrounded"), None);
    assert_eq!(
        parse_short_id("backgrounded · \u{1b}[36mdeadbeef\u{1b}[39m"),
        Some("deadbeef".into())
    );
}

// ---- apply_opts / TurnOpts --------------------------------------------

#[test]
fn turn_opts_default_is_empty() {
    let o = TurnOpts::default();
    assert!(o.model.is_none());
    assert!(o.agent.is_none());
    assert!(o.permission_mode.is_empty());
    assert!(o.settings_json.is_empty());
}

#[test]
fn apply_opts_minimal_only_sets_permission_mode() {
    let mut cmd = std::process::Command::new("claude");
    apply_opts(&mut cmd, &TurnOpts::default());
    assert_eq!(
        args_of(&cmd),
        vec!["--permission-mode", "bypassPermissions"]
    );
    // No env vars applied.
    assert!(cmd.get_envs().next().is_none());
}

#[test]
fn apply_opts_full_ordering_and_env() {
    let opts = TurnOpts {
        model: Some("opus".into()),
        agent: Some("Plan".into()),
        effort: Some("high".into()),
        permission_mode: "acceptEdits".into(),
        settings_json: "{\"s\":1}".into(),
        engine_url: "http://127.0.0.1:9".into(),
        mcp_config: "{\"mcpServers\":{}}".into(),
        session_env_id: "ses_1".into(),
    };
    let mut cmd = std::process::Command::new("claude");
    apply_opts(&mut cmd, &opts);
    assert_eq!(
        args_of(&cmd),
        vec![
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--permission-mode",
            "acceptEdits",
            "--settings",
            "{\"s\":1}",
            "--model",
            "opus",
            "--agent",
            "Plan",
            "--effort",
            "high",
        ]
    );
    let envs: std::collections::HashMap<String, Option<String>> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert_eq!(
        envs.get("OPMAN_ENGINE_URL"),
        Some(&Some("http://127.0.0.1:9".into()))
    );
    assert_eq!(envs.get("OPENCODE_SESSION_ID"), Some(&Some("ses_1".into())));
}

#[test]
fn apply_opts_empty_agent_string_is_omitted() {
    let opts = TurnOpts {
        agent: Some(String::new()),
        ..Default::default()
    };
    let mut cmd = std::process::Command::new("claude");
    apply_opts(&mut cmd, &opts);
    assert!(!args_of(&cmd).iter().any(|a| a == "--agent"));
}

// ---- model_display_name ------------------------------------------------

#[test]
fn model_display_name_cases() {
    assert_eq!(model_display_name("claude-opus-4-8"), "Claude Opus 4.8");
    assert_eq!(
        model_display_name("claude-sonnet-4-5-20250101"),
        "Claude Sonnet 4.5"
    );
    assert_eq!(
        model_display_name("claude-haiku-4-5[1m]"),
        "Claude Haiku 4.5 (1m)"
    );
    // tier-only (no version segments)
    assert_eq!(model_display_name("claude-opus"), "Claude Opus");
    // no claude- prefix still normalizes
    assert_eq!(model_display_name("gpt-4"), "Claude Gpt 4");
}

// ---- model_limits ------------------------------------------------------

#[test]
fn model_limits_by_pattern() {
    assert_eq!(model_limits("claude-opus-4-8"), (1_000_000, 128_000));
    assert_eq!(model_limits("claude-sonnet-5"), (1_000_000, 128_000));
    assert_eq!(model_limits("some-fable-thing"), (1_000_000, 128_000));
    assert_eq!(model_limits("claude-sonnet-4-5"), (200_000, 64_000));
    assert_eq!(model_limits("claude-haiku-4-5"), (200_000, 32_000));
    assert_eq!(model_limits("weird[1m]model"), (1_000_000, 128_000));
    assert_eq!(model_limits("mystery-model"), (200_000, 64_000));
}

// ---- AgentInfo ---------------------------------------------------------

#[test]
fn agent_info_deserializes_with_renamed_fields() {
    let a: AgentInfo = serde_json::from_value(serde_json::json!({
        "id": "sid",
        "sessionId": "uuid-1",
        "cwd": "/proj",
        "kind": "background",
        "state": "working",
        "startedAt": 12345u64,
    }))
    .unwrap();
    assert_eq!(a.id, "sid");
    assert_eq!(a.session_id, "uuid-1");
    assert_eq!(a.started_at, 12345);
    assert!(a.is_busy());
}

#[test]
fn agent_info_defaults_when_fields_missing() {
    let a: AgentInfo = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(a.id.is_empty());
    assert!(a.session_id.is_empty());
    assert!(a.state.is_none());
    assert!(!a.is_busy());
}

// ---- locate_jsonl / locate_subagent_jsonl -----------------------------

#[test]
fn locate_helpers_reject_empty_and_missing() {
    assert!(locate_jsonl("").is_none());
    assert!(locate_subagent_jsonl("").is_none());
    // A well-formed but non-existent uuid resolves to nothing (read-only over home).
    assert!(locate_jsonl("nonexistent-uuid-0000-abc").is_none());
    assert!(locate_subagent_jsonl("nonexistent-agent-0000-abc").is_none());
}

// ---- binary-absent / stubbed-binary branches --------------------------

const MISSING_BIN: &str = "/nonexistent/definitely-not-a-real-claude-bin";

#[test]
fn version_falls_back_when_binary_absent() {
    let v = with_bin(MISSING_BIN, version);
    assert_eq!(v, "claude");
}

#[test]
fn agents_json_ok_empty_with_echo_and_err_when_absent() {
    // `echo agents --json --all` prints non-JSON → parsed as empty list (Ok).
    let ok = with_bin("echo", || agents_json(Some("/some/dir")));
    assert!(ok.unwrap().is_empty());
    // Missing binary → the spawn itself errors.
    let err = with_bin(MISSING_BIN, || agents_json(None));
    assert!(err.is_err());
}

#[test]
fn stop_ok_with_echo_and_err_when_absent() {
    assert!(with_bin("echo", || stop("short1")).is_ok());
    assert!(with_bin(MISSING_BIN, || stop("short1")).is_err());
}

#[test]
fn introspect_returns_default_when_absent_or_no_init() {
    // Missing binary → spawn fails → default.
    let info = with_bin(MISSING_BIN, || introspect("/tmp"));
    assert!(info.commands.is_empty() && info.agents.is_empty());
    // echo emits a non-init line → still default, and the child is reaped cleanly.
    let info = with_bin("echo", || introspect("/tmp"));
    assert!(info.commands.is_empty() && info.agents.is_empty());
}

#[test]
fn fetch_models_none_when_absent() {
    assert!(with_bin(MISSING_BIN, fetch_models_via_cli).is_none());
}

/// Write an executable shell script and return (its path, the tempdir keeping it alive).
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

#[test]
fn fetch_models_parses_fenced_json_result() {
    // Stub prints an opencode-shaped `{"result": "```json\n[...]\n```"}` envelope.
    let (bin, _dir) = make_script(
        r#"printf '%s' '{"result":"```json\n[\"claude-opus-4-8\",\"claude-haiku-4-5-20251001\",\"claude-sonnet-5\"]\n```"}'"#,
    );
    let models = with_bin(&bin, fetch_models_via_cli).expect("parsed models");
    assert_eq!(models.len(), 3);
    assert_eq!(models[0].id, "claude-opus-4-8");
    assert_eq!(models[0].display_name, "Claude Opus 4.8");
    assert_eq!(models[0].context_window, 1_000_000);
    assert_eq!(models[0].max_output, 128_000);
    assert_eq!(models[2].display_name, "Claude Sonnet 5");
}

#[test]
fn fetch_models_none_on_empty_array() {
    let (bin, _dir) = make_script(r#"printf '%s' '{"result":"[]"}'"#);
    assert!(with_bin(&bin, fetch_models_via_cli).is_none());
}

#[test]
fn fetch_models_none_on_nonzero_exit() {
    let (bin, _dir) = make_script("exit 3");
    assert!(with_bin(&bin, fetch_models_via_cli).is_none());
}

#[test]
fn fetch_models_none_when_result_field_missing() {
    let (bin, _dir) = make_script(r#"printf '%s' '{"other":"x"}'"#);
    assert!(with_bin(&bin, fetch_models_via_cli).is_none());
}

#[test]
fn bg_start_errors_when_short_id_unparseable() {
    // `echo` prints the args (no "backgrounded · <id>" line), so short-id parsing fails.
    let opts = TurnOpts::default();
    let r = with_bin("echo", || bg_start("/tmp", &opts, "hello prompt"));
    assert!(r.is_err());
}

#[test]
fn bg_resume_errors_when_short_id_unparseable() {
    let opts = TurnOpts::default();
    let r = with_bin("echo", || {
        bg_resume("/tmp", "uuid-x", &opts, "resume prompt")
    });
    assert!(r.is_err());
}
