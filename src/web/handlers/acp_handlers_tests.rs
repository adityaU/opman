//! Tests for the ACP agent list and the ids it will accept.
//!
//! `validate_id` runs before anything is written, which is the whole point: an id that the
//! config loader would later ignore must be refused while the user is still looking at the
//! form, not accepted into a file and silently dropped.

use super::*;

#[test]
fn ids_are_held_to_the_shape_the_loader_accepts() {
    assert_eq!(validate_id(" gemini ").ok().as_deref(), Some("gemini"));
    assert_eq!(validate_id("opencode-acp").ok().as_deref(), Some("opencode-acp"));
    // An id becomes a runner label, a provider id and a session-file name.
    assert!(validate_id("../escape").is_err());
    assert!(validate_id("Upper").is_err());
    assert!(validate_id("").is_err());
    assert!(validate_id("-leading").is_err());
}

#[test]
fn a_view_reports_env_by_name_only() {
    let entry = AgentConfig {
        command: "gemini-acp".to_string(),
        env: std::collections::BTreeMap::from([("API_KEY".to_string(), "secret".to_string())]),
        ..AgentConfig::default()
    };
    let row = view("gemini", &entry, Liveness { customized: true, ..Liveness::default() });
    assert_eq!(row.env_names, vec!["API_KEY".to_string()]);
    // The serialized row is what reaches the browser; the value must not be in it.
    let json = serde_json::to_string(&row).expect("serialize");
    assert!(!json.contains("secret"), "env value leaked into {json}");
}

#[test]
fn a_launchable_agent_is_one_with_a_command_and_the_switch_on() {
    let disabled = AgentConfig {
        command: "gemini-acp".to_string(),
        enabled: false,
        ..AgentConfig::default()
    };
    assert!(!view("gemini", &disabled, Liveness { customized: true, ..Liveness::default() }).launchable);

    let ready = AgentConfig {
        enabled: true,
        ..disabled
    };
    assert!(view("gemini", &ready, Liveness { customized: true, ..Liveness::default() }).launchable);
}

#[test]
fn builtins_are_marked_so_the_page_offers_restore_rather_than_delete() {
    let entry = AgentConfig {
        command: "npx".to_string(),
        ..AgentConfig::default()
    };
    assert!(view("claude", &entry, Liveness { running: true, ..Liveness::default() }).builtin);
    assert!(view("codex", &entry, Liveness { running: true, ..Liveness::default() }).builtin);
    assert!(!view("gemini", &entry, Liveness::default()).builtin);
}
