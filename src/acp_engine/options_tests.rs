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
fn a_value_the_agent_never_reported_has_no_channel() {
    let s = setup();
    // The guard that stops opman pushing a value the agent would error on.
    assert_eq!(channel_for(&s, MODE, "nonsense"), None);
    assert_eq!(channel_for(&s, MODEL, "nonsense"), None);
    assert_eq!(channel_for(&s, "nope", "anything"), None);
}

/// Modes published the spec's way are set the spec's way. This is the bug the `Channel` enum
/// exists for: every choice used to go out as `session/set_config_option`, which an agent
/// serving `session/set_mode` answers with "method not found" — leaving a mode picker that
/// changed nothing and said nothing.
#[test]
fn spec_modes_are_set_with_set_mode() {
    let s = setup();
    // `bypassPermissions` exists only in the `modes` block, which is also why the channel
    // has to consult it: missing it would reject the very mode opman starts Claude in.
    assert_eq!(
        channel_for(&s, MODE, "bypassPermissions"),
        Some(Channel::Mode)
    );
    assert_eq!(channel_for(&s, MODE, "acceptEdits"), Some(Channel::Mode));
    assert_eq!(Channel::Mode.method(), "session/set_mode");
}

/// With no `modes` block the same value is a config option, and goes out generically.
#[test]
fn config_only_modes_fall_back_to_the_generic_setter() {
    let s = setup_without("modes");
    assert_eq!(channel_for(&s, MODE, "acceptEdits"), Some(Channel::Config));
    assert_eq!(channel_for(&s, MODEL, "sonnet"), Some(Channel::Config));
}

/// Claude reports no `models` block at all, so its models are set generically even though its
/// modes are not — the two halves of one reply can use different channels.
#[test]
fn models_and_modes_can_use_different_channels() {
    let s = setup();
    assert_eq!(channel_for(&s, MODE, "acceptEdits"), Some(Channel::Mode));
    assert_eq!(channel_for(&s, MODEL, "sonnet"), Some(Channel::Config));
}

#[test]
fn spec_models_are_set_with_set_model() {
    let mut s = setup();
    s["models"] = json!({
        "currentModelId": "gpt-5",
        "availableModels": [{ "modelId": "gpt-5", "name": "GPT-5" }],
    });
    assert_eq!(channel_for(&s, MODEL, "gpt-5"), Some(Channel::Model));
    assert_eq!(Channel::Model.method(), "session/set_model");
}

/// Each method names the value its own way; sending a `configId` to `set_mode` would be a
/// request the agent cannot read.
#[test]
fn each_channel_names_its_value_the_way_its_method_expects() {
    assert_eq!(
        Channel::Mode.params("s1", MODE, "acceptEdits"),
        json!({ "sessionId": "s1", "modeId": "acceptEdits" })
    );
    assert_eq!(
        Channel::Model.params("s1", MODEL, "sonnet"),
        json!({ "sessionId": "s1", "modelId": "sonnet" })
    );
    assert_eq!(
        Channel::Config.params("s1", EFFORT, "high"),
        json!({ "sessionId": "s1", "configId": "effort", "value": "high" })
    );
}

/// `session/set_mode` and `session/set_model` answer with nothing, so what the agent accepted
/// has to be written down here — otherwise the next sync compares against a stale current
/// value and pushes the same choice again on every turn.
#[test]
fn a_spec_choice_is_recorded_because_its_reply_carries_nothing() {
    let mut s = setup();
    assert_eq!(selected(&s, MODE).as_deref(), Some("default"));
    note_current(&mut s, Channel::Mode, "acceptEdits");
    assert_eq!(selected(&s, MODE).as_deref(), Some("acceptEdits"));
}

/// The generic channel's reply carries the whole reconciled list, so there is nothing here to
/// write down and nothing to overwrite.
#[test]
fn the_generic_channel_records_nothing_itself() {
    let mut s = setup();
    note_current(&mut s, Channel::Config, "sonnet");
    assert_eq!(selected(&s, MODEL).as_deref(), Some("opus[1m]"));
}

/// What "current" means depends on where the agent published the option.
#[test]
fn selected_reads_whichever_place_the_agent_used() {
    // Claude: mode from the `modes` block, model from configOptions.
    assert_eq!(selected(&setup(), MODE).as_deref(), Some("default"));
    assert_eq!(selected(&setup(), MODEL).as_deref(), Some("opus[1m]"));
    assert_eq!(selected(&setup(), EFFORT).as_deref(), Some("medium"));

    let mut spec = setup();
    spec["models"] = json!({ "currentModelId": "gpt-5", "availableModels": [] });
    assert_eq!(selected(&spec, MODEL).as_deref(), Some("gpt-5"));
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
