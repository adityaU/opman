//! What each mechanism hands the agent.

use serde_json::{json, Value};

use super::super::config::AgentConfig;
use super::super::native_tools::NativeTools;
use super::super::native_tools_catalog::tool_set_for;

fn agent(id: &str, setting: NativeTools) -> AgentConfig {
    AgentConfig {
        native_tools: setting,
        tool_set: tool_set_for(id),
        ..AgentConfig::default()
    }
}

fn off(ids: &[&str]) -> NativeTools {
    NativeTools::Disabled(ids.iter().map(|id| (*id).to_string()).collect())
}

fn env_json(agent: &AgentConfig) -> (&'static str, Value) {
    let (key, text) = agent.tool_env().expect("env");
    (key, serde_json::from_str(&text).expect("valid json"))
}

#[test]
fn everything_on_renders_nothing() {
    for id in ["claude", "opencode", "codex"] {
        let all = agent(id, NativeTools::All);
        assert!(all.session_meta().is_none(), "{id}");
        assert!(all.tool_env().is_none(), "{id}");
    }
}

#[test]
fn an_agent_with_no_mechanism_renders_nothing_even_if_a_choice_is_stored() {
    let stray = AgentConfig {
        native_tools: NativeTools::Off,
        ..AgentConfig::default()
    };
    assert!(stray.session_meta().is_none());
    assert!(stray.tool_env().is_none());
}

#[test]
fn claude_gets_disallowed_tools_in_session_meta_and_no_env() {
    let claude = agent("claude", off(&["Bash", "WebFetch"]));
    assert_eq!(
        claude.session_meta(),
        Some(json!({"claudeCode":{"options":{"disallowedTools":["Bash","WebFetch"]}}}))
    );
    assert!(claude.tool_env().is_none());
}

#[test]
fn claude_off_lists_every_tool() {
    let meta = agent("claude", NativeTools::Off)
        .session_meta()
        .expect("meta");
    let list = &meta["claudeCode"]["options"]["disallowedTools"];
    assert_eq!(
        list.as_array().map(Vec::len),
        tool_set_for("claude").map(|s| s.tools.len())
    );
}

#[test]
fn opencode_denies_the_chosen_permissions() {
    let opencode = agent("opencode", off(&["bash", "edit"]));
    let (key, value) = env_json(&opencode);
    assert_eq!(key, "OPENCODE_CONFIG_CONTENT");
    assert_eq!(value, json!({"permission":{"bash":"deny","edit":"deny"}}));
    assert!(opencode.session_meta().is_none());
}

#[test]
fn opencode_keeps_the_users_own_config_and_denial_wins() {
    let mut opencode = agent("opencode", off(&["bash"]));
    opencode.env.insert(
        "OPENCODE_CONFIG_CONTENT".to_string(),
        r#"{"model":"x/y","permission":{"bash":"ask","edit":"ask"}}"#.to_string(),
    );
    let (_, value) = env_json(&opencode);
    assert_eq!(value["model"], "x/y");
    assert_eq!(value["permission"]["bash"], "deny");
    assert_eq!(value["permission"]["edit"], "ask");
}

#[test]
fn opencode_blanket_permission_string_is_kept_as_a_star_rule() {
    let mut opencode = agent("opencode", off(&["bash"]));
    opencode.env.insert(
        "OPENCODE_CONFIG_CONTENT".to_string(),
        r#"{"permission":"allow"}"#.to_string(),
    );
    let (_, value) = env_json(&opencode);
    assert_eq!(value["permission"], json!({"*":"allow","bash":"deny"}));
}

#[test]
fn a_broken_user_value_is_replaced_not_crashed_on() {
    let mut opencode = agent("opencode", off(&["bash"]));
    opencode.env.insert(
        "OPENCODE_CONFIG_CONTENT".to_string(),
        "not json".to_string(),
    );
    let (_, value) = env_json(&opencode);
    assert_eq!(value, json!({"permission":{"bash":"deny"}}));
}

#[test]
fn codex_nests_config_paths_and_uses_a_mode_for_web_search() {
    let codex = agent("codex", NativeTools::Off);
    let (key, value) = env_json(&codex);
    assert_eq!(key, "CODEX_CONFIG");
    assert_eq!(
        value,
        json!({
            "features": {"shell_tool": false, "image_generation": false},
            "web_search": "disabled"
        })
    );
}

#[test]
fn codex_merges_over_the_users_config() {
    let mut codex = agent("codex", off(&["features.shell_tool"]));
    codex.env.insert(
        "CODEX_CONFIG".to_string(),
        r#"{"features":{"hooks":true},"model":"m"}"#.to_string(),
    );
    let (_, value) = env_json(&codex);
    assert_eq!(
        value["features"],
        json!({"hooks": true, "shell_tool": false})
    );
    assert_eq!(value["model"], "m");
}

#[test]
fn antigravity_gets_disabled_tools_in_session_meta_and_no_env() {
    let antigravity = agent("antigravity", off(&["run_command", "run_workflow"]));
    assert_eq!(
        antigravity.session_meta(),
        Some(json!({"antigravity":{"disabledTools":["run_command","run_workflow"]}}))
    );
    assert!(antigravity.tool_env().is_none());
}

#[test]
fn antigravity_off_lists_every_tool_and_all_on_lists_none() {
    let meta = agent("antigravity", NativeTools::Off)
        .session_meta()
        .expect("meta");
    let list = &meta["antigravity"]["disabledTools"];
    assert_eq!(
        list.as_array().map(Vec::len),
        tool_set_for("antigravity").map(|s| s.tools.len())
    );
    assert!(agent("antigravity", NativeTools::All).session_meta().is_none());
}
