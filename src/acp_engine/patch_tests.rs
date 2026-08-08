//! Tests for the on-disk half of the agent registry.

use super::*;

/// A patch is a set of decisions, not a struct of values: what it does not mention must
/// survive, and what it mentions must land even when the value is the type's default.
#[test]
fn absent_fields_are_left_alone_and_present_ones_land() {
    let mut target = AgentConfig {
        display_name: "Claude".to_string(),
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "pkg".to_string()],
        default_mode: "bypassPermissions".to_string(),
        ..AgentConfig::default()
    };
    AgentPatch {
        args: Some(Vec::new()),
        default_mode: Some(String::new()),
        enabled: Some(false),
        ..AgentPatch::default()
    }
    .apply(&mut target);

    assert_eq!(target.display_name, "Claude");
    assert_eq!(target.command, "npx");
    assert!(target.args.is_empty());
    assert!(target.default_mode.is_empty());
    assert!(!target.enabled);
}

/// Absent fields must not appear in the written file. A patch that spelled every default
/// out would pin the agent to today's built-in and stop tracking opman's own updates.
#[test]
fn only_decided_fields_are_written() {
    let document = AcpDocument {
        agents: BTreeMap::from([(
            "claude".to_string(),
            AgentPatch {
                enabled: Some(false),
                ..AgentPatch::default()
            },
        )]),
    };
    let json = serde_json::to_string(&document).expect("serialize");
    assert_eq!(json, r#"{"agents":{"claude":{"enabled":false}}}"#);
}

#[test]
fn a_patch_round_trips_through_json() {
    let patch = AgentPatch {
        command: Some("gemini-acp".to_string()),
        args: Some(vec!["--stdio".to_string()]),
        env: Some(BTreeMap::from([("KEY".to_string(), "v".to_string())])),
        client_caps: Some(ClientCaps {
            read_text_file: true,
            write_text_file: false,
            terminal: true,
        }),
        modes_are_agents: Some(true),
        ..AgentPatch::default()
    };
    let json = serde_json::to_string(&patch).expect("serialize");
    let back: AgentPatch = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, patch);
}

#[test]
fn an_untouched_patch_is_empty() {
    assert!(AgentPatch::default().is_empty());
    assert!(!AgentPatch {
        enabled: Some(true),
        ..AgentPatch::default()
    }
    .is_empty());
}
