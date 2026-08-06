//! Session configuration discovered from the agent, not assumed by opman.
//!
//! Models, permission modes and effort levels are per-agent facts. ACP hands them over in
//! `session/new`'s reply — as `configOptions` (a generic id/options list), as the spec's
//! `modes`, or as the experimental `models` — so opman reads all three and asks the agent
//! what it supports instead of shipping a table that goes stale. This is what lets a new
//! ACP server appear in the engine picker with its own real choices and no code change.

use serde_json::{json, Value};

/// Well-known `configOptions` ids. ACP does not reserve these, but they are the ids agents
/// use in practice, and each maps onto a control opman already has.
pub const MODE: &str = "mode";
pub const MODEL: &str = "model";
pub const EFFORT: &str = "effort";

/// One selectable value.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Whether the agent offers `value` for `option_id`. Checked before sending, so opman never
/// pushes a mode or model the agent would reject.
pub fn offers(setup: &Value, option_id: &str, value: &str) -> bool {
    if option_id == MODE && mode_ids(setup).iter().any(|m| m.id == value) {
        return true;
    }
    choices(setup, option_id).iter().any(|c| c.id == value)
}

/// The values for one `configOptions` entry.
pub fn choices(setup: &Value, option_id: &str) -> Vec<Choice> {
    let Some(options) = setup.get("configOptions").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(entry) = options
        .iter()
        .find(|o| o.get("id").and_then(Value::as_str) == Some(option_id))
    else {
        return Vec::new();
    };
    entry
        .get("options")
        .and_then(Value::as_array)
        .map(|values| values.iter().map(choice_from_value).collect())
        .unwrap_or_default()
}

/// The currently selected value for a `configOptions` entry, if the agent reported one.
pub fn current(setup: &Value, option_id: &str) -> Option<String> {
    let options = setup.get("configOptions")?.as_array()?;
    let entry = options
        .iter()
        .find(|o| o.get("id").and_then(Value::as_str) == Some(option_id))?;
    entry
        .get("currentValue")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Permission modes: the spec's `modes.availableModes` first, falling back to the `mode`
/// config option for agents that only expose it that way.
pub fn mode_ids(setup: &Value) -> Vec<Choice> {
    let listed = setup
        .get("modes")
        .and_then(|m| m.get("availableModes"))
        .and_then(Value::as_array);
    if let Some(modes) = listed {
        return modes.iter().map(choice_from_mode).collect();
    }
    choices(setup, MODE)
}

/// The mode the agent says it is in.
pub fn current_mode(setup: &Value) -> Option<String> {
    setup
        .get("modes")
        .and_then(|m| m.get("currentModeId"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| current(setup, MODE))
}

/// Models: the experimental `models.availableModels` first, else the `model` config option.
/// Claude reports null for the former and a full list in the latter, so both are needed.
pub fn models(setup: &Value) -> Vec<Choice> {
    let listed = setup
        .get("models")
        .and_then(|m| m.get("availableModels"))
        .and_then(Value::as_array);
    if let Some(models) = listed {
        return models.iter().map(choice_from_model).collect();
    }
    choices(setup, MODEL)
}

/// Model catalog in the opencode `/provider` shape, so the engine picker renders an ACP
/// agent's models with no special casing.
///
/// `permission_modes` rides along on the same payload rather than getting an endpoint of
/// its own. The picker's permission list was a table keyed by runner name, which a
/// config-declared agent could never appear in; carrying the agent's own modes here means a
/// new ACP server arrives with its real modes and no frontend change.
pub fn provider_payload(
    agent_id: &str,
    display_name: &str,
    models: &[Choice],
    current_model: Option<&str>,
    permission_modes: &[Choice],
) -> Value {
    let entries: serde_json::Map<String, Value> = models
        .iter()
        // "default" is an alias for whatever the agent would pick anyway; listing it beside
        // the concrete models it resolves to only makes the picker ambiguous.
        .filter(|m| m.id != "default")
        .map(|m| {
            let entry = json!({
                "id": m.id,
                "providerID": agent_id,
                "name": if m.name.is_empty() { &m.id } else { &m.name },
                "description": m.description,
                "limit": { "context": 0, "output": 0 },
            });
            (m.id.clone(), entry)
        })
        .collect();
    let modes: Vec<Value> = permission_modes
        .iter()
        .map(|m| {
            json!({
                "value": m.id,
                "label": if m.name.is_empty() { &m.id } else { &m.name },
                "description": m.description,
            })
        })
        .collect();
    // The agent's selected model, unless it is the `default` alias — which names no entry in
    // the list above and would leave the picker pointing at nothing.
    let default = current_model
        .filter(|id| !id.is_empty() && *id != "default" && entries.contains_key(*id))
        .map(|id| json!({ agent_id: id }))
        .unwrap_or_else(|| json!({}));
    json!({
        "all": [{ "id": agent_id, "name": display_name, "models": entries }],
        "connected": [agent_id],
        "default": default,
        "permissionModes": modes,
    })
}

fn choice_from_value(value: &Value) -> Choice {
    Choice {
        id: string_at(value, "value"),
        name: string_at(value, "name"),
        description: string_at(value, "description"),
    }
}

fn choice_from_mode(value: &Value) -> Choice {
    Choice {
        id: string_at(value, "id"),
        name: string_at(value, "name"),
        description: string_at(value, "description"),
    }
}

fn choice_from_model(value: &Value) -> Choice {
    Choice {
        id: string_at(value, "modelId"),
        name: string_at(value, "name"),
        description: string_at(value, "description"),
    }
}

fn string_at(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod options_tests;
