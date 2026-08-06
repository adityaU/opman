//! Tests for what opman reads out of a `session/new` reply.
//!
//! The fixture below is the real (trimmed) shape returned by Claude's ACP adapter: the
//! spec's `modes` block is populated, `models` is absent, and the concrete model list only
//! exists as a `configOptions` entry. Every fallback in `options.rs` exists because of that
//! asymmetry, so the tests exercise the fixture as-is and then strip parts of it.

use super::*;

fn setup() -> Value {
    json!({
      "sessionId": "abc",
      "modes": {"currentModeId":"default","availableModes":[
         {"id":"default","name":"Manual","description":"Standard behavior"},
         {"id":"acceptEdits","name":"Accept Edits","description":"Auto-accept file edits"},
         {"id":"bypassPermissions","name":"Bypass Permissions","description":"Bypass all checks"}]},
      "configOptions": [
        {"id":"mode","name":"Mode","currentValue":"default","options":[
           {"value":"default","name":"Manual"},{"value":"acceptEdits","name":"Accept Edits"}]},
        {"id":"model","name":"Model","currentValue":"opus[1m]","options":[
           {"value":"default","name":"Default (recommended)","description":"Opus 5"},
           {"value":"opus[1m]","name":"Opus (1M context)","description":"Opus 5 with 1M context"},
           {"value":"sonnet","name":"Sonnet","description":"Efficient"}]},
        {"id":"effort","name":"Effort","currentValue":"medium","options":[
           {"value":"low","name":"Low"},{"value":"high","name":"High"}]}
      ]
    })
}

/// The fixture with one top-level key dropped, to exercise the fallback paths.
fn setup_without(key: &str) -> Value {
    let mut v = setup();
    v.as_object_mut().expect("object").remove(key);
    v
}

fn ids(choices: &[Choice]) -> Vec<&str> {
    choices.iter().map(|c| c.id.as_str()).collect()
}

#[test]
fn models_fall_back_to_the_model_config_option() {
    let listed = models(&setup());
    // Claude reports no experimental `models` block, so the only place its real model list
    // appears is the generic `configOptions` entry. Order is the agent's own.
    assert_eq!(ids(&listed), vec!["default", "opus[1m]", "sonnet"]);
    assert_eq!(listed[1].name, "Opus (1M context)");
    assert_eq!(listed[2].description, "Efficient");
}

#[test]
fn spec_models_take_precedence_over_the_config_option() {
    let mut s = setup();
    s["models"] = json!({"availableModels":[{"modelId":"m1","name":"M1"}]});
    // An agent that implements the spec's `models` is authoritative; the config option is
    // only a stand-in for agents that do not.
    assert_eq!(ids(&models(&s)), vec!["m1"]);
    assert_eq!(models(&s)[0].name, "M1");
}

#[test]
fn mode_ids_prefer_the_spec_modes_block() {
    let from_spec = mode_ids(&setup());
    assert_eq!(
        ids(&from_spec),
        vec!["default", "acceptEdits", "bypassPermissions"]
    );
    assert_eq!(from_spec[2].name, "Bypass Permissions");

    // Without `modes`, the `mode` config option carries the list — and it is a shorter one
    // here, which is exactly why the spec block wins when both are present.
    assert_eq!(
        ids(&mode_ids(&setup_without("modes"))),
        vec!["default", "acceptEdits"]
    );
}

#[test]
fn current_mode_prefers_current_mode_id() {
    assert_eq!(current_mode(&setup()).as_deref(), Some("default"));
    // Fallback for agents that only expose the mode as a config option.
    assert_eq!(
        current_mode(&setup_without("modes")).as_deref(),
        Some("default")
    );
    let mut s = setup_without("modes");
    s["configOptions"][0]["currentValue"] = json!("acceptEdits");
    assert_eq!(current_mode(&s).as_deref(), Some("acceptEdits"));
}

#[test]
fn current_reads_the_selected_config_option_value() {
    assert_eq!(current(&setup(), MODEL).as_deref(), Some("opus[1m]"));
    assert_eq!(current(&setup(), EFFORT).as_deref(), Some("medium"));
    // An option the agent never reported has no current value to report either.
    assert!(current(&setup(), "nope").is_none());
}

#[test]
fn offers_accepts_only_values_the_agent_reported() {
    let s = setup();
    // `bypassPermissions` exists only in the `modes` block, not in configOptions — the
    // reason `offers` consults modes as well. Missing it would make opman reject the very
    // mode it wants to start Claude in.
    assert!(offers(&s, MODE, "bypassPermissions"));
    assert!(offers(&s, MODE, "acceptEdits"));
    assert!(offers(&s, MODEL, "sonnet"));
    // The guard that stops opman pushing a value the agent would error on.
    assert!(!offers(&s, MODE, "nonsense"));
    assert!(!offers(&s, MODEL, "nonsense"));
}

#[test]
fn provider_payload_shapes_models_like_the_picker_expects() {
    let payload = provider_payload(
        "claude",
        "Claude",
        &models(&setup()),
        current(&setup(), MODEL).as_deref(),
        &mode_ids(&setup()),
    );
    // The picker reads `all`/`connected`/`default`, the same shape the other engines emit —
    // a `providers` key here would have rendered an empty model list.
    let provider = &payload["all"][0];
    assert_eq!(provider["id"], "claude");
    assert_eq!(provider["name"], "Claude");
    assert_eq!(payload["connected"][0], "claude");
    assert_eq!(payload["default"]["claude"], "opus[1m]");

    let entries = provider["models"].as_object().expect("models map");
    // "default" is an alias for whatever the agent would have chosen; showing it next to
    // the model it resolves to makes the picker ambiguous, so it is dropped.
    assert!(
        !entries.contains_key("default"),
        "keys: {:?}",
        entries.keys().collect::<Vec<_>>()
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(entries["opus[1m]"]["id"], "opus[1m]");
    assert_eq!(entries["opus[1m]"]["name"], "Opus (1M context)");
    assert_eq!(entries["sonnet"]["description"], "Efficient");
    assert_eq!(entries["sonnet"]["providerID"], "claude");
}

#[test]
fn provider_payload_carries_the_agents_own_permission_modes() {
    let modes = vec![
        Choice {
            id: "bypassPermissions".to_string(),
            name: "Bypass Permissions".to_string(),
            description: "Bypass all checks".to_string(),
        },
        // An agent that lists a mode with no display name still needs a usable label, so
        // the id doubles as one rather than rendering an empty row.
        Choice {
            id: "plan".to_string(),
            name: String::new(),
            description: String::new(),
        },
    ];
    let payload = provider_payload("claude", "Claude", &[], None, &modes);
    let listed = payload["permissionModes"].as_array().expect("array");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0]["value"], "bypassPermissions");
    assert_eq!(listed[0]["label"], "Bypass Permissions");
    assert_eq!(listed[0]["description"], "Bypass all checks");
    assert_eq!(listed[1]["value"], "plan");
    assert_eq!(listed[1]["label"], "plan");
}
