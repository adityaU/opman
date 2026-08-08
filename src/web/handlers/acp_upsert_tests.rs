//! Tests for the patch semantics of an agent write.

use super::*;

fn body(json: serde_json::Value) -> UpsertAgent {
    serde_json::from_value(json).expect("deserialize body")
}

/// The property the whole editor rests on: a form that only knows about one field must not
/// erase the ones it never showed.
#[test]
fn an_absent_field_leaves_the_entry_alone() {
    let mut target = AgentPatch {
        command: Some("gemini-acp".to_string()),
        default_mode: Some("plan".to_string()),
        ..AgentPatch::default()
    };
    apply(&mut target, body(serde_json::json!({ "enabled": false }))).expect("apply");
    assert_eq!(target.command.as_deref(), Some("gemini-acp"));
    assert_eq!(target.default_mode.as_deref(), Some("plan"));
    assert_eq!(target.enabled, Some(false));
}

/// The other half of the same property: an empty value is a decision, and must be written
/// down rather than treated as "nothing was said".
#[test]
fn an_explicit_empty_value_is_recorded() {
    let mut target = AgentPatch::default();
    apply(
        &mut target,
        body(serde_json::json!({ "args": [], "defaultMode": "" })),
    )
    .expect("apply");
    assert_eq!(target.args, Some(Vec::new()));
    assert_eq!(target.default_mode, Some(String::new()));
}

#[test]
fn env_is_edited_by_name_so_unseen_values_survive() {
    let mut target = AgentPatch {
        env: Some(BTreeMap::from([
            ("KEEP".to_string(), "kept".to_string()),
            ("DROP".to_string(), "gone".to_string()),
        ])),
        ..AgentPatch::default()
    };
    apply(
        &mut target,
        body(serde_json::json!({
            "envSet": { "ADDED": "new" },
            "envUnset": ["DROP"],
        })),
    )
    .expect("apply");
    let env = target.env.expect("env");
    assert_eq!(env.get("KEEP").map(String::as_str), Some("kept"));
    assert_eq!(env.get("ADDED").map(String::as_str), Some("new"));
    assert!(!env.contains_key("DROP"));
}

/// An entry whose map ends up empty should lose the field entirely — an empty object in
/// `acp.json` says nothing and only invites the reader to wonder what it meant.
#[test]
fn clearing_every_variable_drops_the_env_field() {
    let mut target = AgentPatch {
        env: Some(BTreeMap::from([("ONLY".to_string(), "v".to_string())])),
        ..AgentPatch::default()
    };
    apply(
        &mut target,
        body(serde_json::json!({ "envUnset": ["ONLY"] })),
    )
    .expect("apply");
    assert_eq!(target.env, None);
}

#[test]
fn a_malformed_runner_slot_is_refused() {
    let mut target = AgentPatch::default();
    // The slot is a label every persisted session is stored against.
    assert!(apply(
        &mut target,
        body(serde_json::json!({ "runner": "../escape" }))
    )
    .is_err());
    assert!(apply(&mut target, body(serde_json::json!({ "runner": "Upper" }))).is_err());
    // Empty means "use the agent id", which the loader fills in.
    assert!(apply(&mut target, body(serde_json::json!({ "runner": "" }))).is_ok());
}
