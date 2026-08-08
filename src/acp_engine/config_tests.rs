//! Tests for the declarative agent registry.
//!
//! `config_path()` reads the process-global `OPMAN_ACP_CONFIG`, and cargo runs tests in
//! the same process on many threads — so every test that points that variable at a
//! fixture takes `ENV_LOCK` first. A mutex is preferred over cramming everything into one
//! `#[test]` because each behaviour then fails on its own, with its own name.

use super::*;

use std::io::Write as _;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Write `body` to a temp file and load the config with `OPMAN_ACP_CONFIG` pointing at it.
/// The `TempDir` is returned only so the caller can keep it alive; dropping it deletes the
/// fixture.
fn load_with(body: &str) -> (AcpConfig, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("acp.json");
    let mut f = std::fs::File::create(&path).expect("create fixture");
    f.write_all(body.as_bytes()).expect("write fixture");
    std::env::set_var("OPMAN_ACP_CONFIG", &path);
    let cfg = load();
    std::env::remove_var("OPMAN_ACP_CONFIG");
    (cfg, dir)
}

/// Load with `OPMAN_ACP_CONFIG` aimed at a path that does not exist.
fn load_without_file() -> AcpConfig {
    let dir = tempfile::tempdir().expect("tempdir");
    std::env::set_var("OPMAN_ACP_CONFIG", dir.path().join("missing.json"));
    let cfg = load();
    std::env::remove_var("OPMAN_ACP_CONFIG");
    cfg
}

#[test]
fn builtin_claude_survives_a_missing_config_file() {
    let _guard = ENV_LOCK.lock().unwrap();
    let cfg = load_without_file();
    // A fresh install ships no acp.json at all; Claude must still be usable.
    let claude = cfg.agents.get("claude").expect("built-in claude entry");
    assert!(
        claude.launchable(),
        "claude should launch with no config file"
    );
    assert!(cfg.active().any(|(id, _)| id == "claude"));
}

#[test]
fn builtin_claude_uses_the_renamed_acp_package() {
    let _guard = ENV_LOCK.lock().unwrap();
    let cfg = load_without_file();
    let claude = cfg.agents.get("claude").expect("built-in claude entry");
    // `@zed-industries/claude-code-acp` is the deprecated name; spawning it regresses to an
    // adapter that no longer receives fixes.
    assert!(
        claude
            .args
            .iter()
            .any(|a| a.contains("@agentclientprotocol/claude-agent-acp")),
        "expected the renamed package in args, got {:?}",
        claude.args
    );
    assert_eq!(claude.runner, "claude");
    // Claude's ACP sessionId doubles as a transcript UUID, which is what makes nested
    // Task subagent sessions readable.
    assert!(claude.subagent_transcripts);
    assert_eq!(claude.default_mode, "bypassPermissions");
}

/// Codex used to be a compile-time runner driving `codex app-server` over its own
/// JSON-RPC. It is an ACP agent now, so "is Codex available" has to be answerable from
/// config alone — with no config file present.
#[test]
fn builtin_codex_ships_as_an_acp_agent() {
    let _guard = ENV_LOCK.lock().unwrap();
    let cfg = load_without_file();
    let codex = cfg.agents.get("codex").expect("built-in codex entry");
    assert!(codex.launchable());
    assert!(
        codex
            .args
            .iter()
            .any(|a| a.contains("@agentclientprotocol/codex-acp")),
        "expected the renamed adapter package in args, got {:?}",
        codex.args
    );
    // The runner slot keeps the name every persisted session and UI label already uses.
    assert_eq!(codex.runner, "codex");
    // Codex's ACP modes are approval policies, so they belong in the permission
    // dropdown rather than the agent picker.
    assert!(!codex.modes_are_agents);
    // The adapter opens on `agent`; restating it here would just drift from the adapter.
    assert!(codex.default_mode.is_empty());
    // Only Claude writes the transcripts the subagent enrichment reads.
    assert!(!codex.subagent_transcripts);
    assert_eq!(
        cfg.for_runner("codex").map(|(id, _)| id.as_str()),
        Some("codex")
    );
}

/// The distinction the patch document exists for: an explicit empty value is a decision,
/// where the previous merge could not tell it from silence and would restore the built-in.
#[test]
fn an_explicit_empty_value_clears_the_builtin_one() {
    let _guard = ENV_LOCK.lock().unwrap();
    let (cfg, _dir) = load_with(r#"{"agents":{"claude":{"args":[],"defaultMode":""}}}"#);
    let claude = cfg.agents.get("claude").expect("claude entry");
    assert!(claude.args.is_empty(), "got {:?}", claude.args);
    assert!(claude.default_mode.is_empty());
    // Clearing two fields must not disturb the rest of the built-in definition.
    assert!(!claude.command.is_empty());
    assert!(claude.subagent_transcripts);
}

#[test]
fn partial_user_entry_keeps_the_builtin_launch_command() {
    let _guard = ENV_LOCK.lock().unwrap();
    let baseline = load_without_file();
    let expected = baseline.agents.get("claude").expect("built-in").clone();

    let (cfg, _dir) = load_with(r#"{"agents":{"claude":{"enabled":false}}}"#);
    let claude = cfg.agents.get("claude").expect("claude entry");
    // The merge property the whole config design rests on: a one-field override must not
    // blank out the fields it did not mention, or disabling an agent would also erase how
    // to launch it.
    assert_eq!(claude.command, expected.command);
    assert_eq!(claude.args, expected.args);
    assert!(!claude.enabled);
    assert!(!claude.launchable());
    assert!(
        !cfg.active().any(|(id, _)| id == "claude"),
        "a disabled agent must drop out of active()"
    );
}

#[test]
fn new_agent_entry_gets_id_derived_defaults() {
    let _guard = ENV_LOCK.lock().unwrap();
    let (cfg, _dir) = load_with(r#"{"agents":{"homegrown":{"command":"homegrown-acp"}}}"#);
    let (_, homegrown) = cfg
        .active()
        .find(|(id, _)| *id == "homegrown")
        .expect("config-declared agent should be active");
    // Adding an ACP server is meant to be a one-line config edit, so the label and the
    // runner slot both fall back to the agent id.
    assert_eq!(homegrown.display_name, "homegrown");
    assert_eq!(homegrown.runner, "homegrown");
    assert_eq!(homegrown.command, "homegrown-acp");
}

#[test]
fn modes_are_agents_is_opt_in_per_agent() {
    let _guard = ENV_LOCK.lock().unwrap();
    let (cfg, _dir) = load_with(
        r#"{"agents":{"opencode-acp":{"command":"opencode","args":["acp"],"modesAreAgents":true}}}"#,
    );
    let entry = cfg.agents.get("opencode-acp").expect("entry");
    assert!(entry.modes_are_agents);
    // Claude fills the same ACP slot with real permission modes, so the default must not
    // move with it.
    let claude = cfg.agents.get("claude").expect("built-in claude");
    assert!(!claude.modes_are_agents);
}

#[test]
fn empty_command_is_not_launchable() {
    // Enabled but with nothing to spawn: the engine would have no process to talk to.
    let entry = AgentConfig {
        enabled: true,
        command: String::new(),
        ..AgentConfig::default()
    };
    assert!(!entry.launchable());
    assert!(AgentConfig {
        command: "homegrown-acp".to_string(),
        ..entry
    }
    .launchable());
}

#[test]
fn malformed_config_is_ignored_rather_than_fatal() {
    let _guard = ENV_LOCK.lock().unwrap();
    let (cfg, _dir) = load_with("{ this is not json");
    // A typo in acp.json must not take opman's default agent down with it.
    let claude = cfg.agents.get("claude").expect("built-in claude survives");
    assert!(claude.launchable());
}

#[test]
fn for_runner_maps_a_runner_slot_to_its_agent() {
    let _guard = ENV_LOCK.lock().unwrap();
    let cfg = load_without_file();
    let (id, _) = cfg.for_runner("claude").expect("claude runner slot");
    assert_eq!(id, "claude");
    assert!(cfg.for_runner("nope").is_none());
}

#[test]
fn env_removals_always_strips_claudecode() {
    let _guard = ENV_LOCK.lock().unwrap();
    let (cfg, _dir) = load_with(r#"{"agents":{"claude":{"envRemove":["MY_VAR"]}}}"#);
    let claude = cfg.agents.get("claude").expect("claude entry");
    let removals: Vec<&str> = claude.env_removals().collect();
    // Claude's adapter refuses `session/new` ("cannot be launched inside another Claude
    // Code session") when it inherits CLAUDECODE, so the built-in list is unconditional
    // and config additions are appended, never substituted.
    assert!(removals.contains(&"CLAUDECODE"), "got {removals:?}");
    assert!(removals.contains(&"MY_VAR"), "got {removals:?}");
}
