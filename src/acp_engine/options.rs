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

/// Which request sets a choice on the agent.
///
/// ACP has three ways of publishing the same idea and each is set by its own method, so where
/// a value was found is also what decides how to send it. This is an enum rather than the
/// boolean it replaced because the boolean made every caller assume `set_config_option`:
/// an agent publishing the spec's `modes` answered that with "method not found", opman logged
/// it at debug, and the picker silently changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// `session/set_mode`, for the spec's `modes.availableModes`.
    Mode,
    /// `session/set_model`, for the spec's `models.availableModels`.
    Model,
    /// `session/set_config_option`, for the generic `configOptions` list.
    Config,
}

impl Channel {
    pub const fn method(self) -> &'static str {
        match self {
            Self::Mode => "session/set_mode",
            Self::Model => "session/set_model",
            Self::Config => "session/set_config_option",
        }
    }

    /// The request body. The three methods name the same value differently.
    pub fn params(self, acp_session: &str, option_id: &str, value: &str) -> Value {
        match self {
            Self::Mode => json!({ "sessionId": acp_session, "modeId": value }),
            Self::Model => json!({ "sessionId": acp_session, "modelId": value }),
            Self::Config => {
                json!({ "sessionId": acp_session, "configId": option_id, "value": value })
            }
        }
    }
}

/// How to set `value` for `option_id`, or `None` when the agent never offered it. Checked
/// before sending, so opman never pushes a mode or model the agent would reject.
pub fn channel_for(setup: &Value, option_id: &str, value: &str) -> Option<Channel> {
    let spec = match option_id {
        MODE => spec_modes(setup).map(|listed| (Channel::Mode, listed)),
        MODEL => spec_models(setup).map(|listed| (Channel::Model, listed)),
        _ => None,
    };
    // Publishing it the spec's way settles the question: that method is the one the agent
    // is guaranteed to answer, even where it also mirrors the value into `configOptions`.
    if let Some((channel, listed)) = spec {
        return listed.iter().any(|c| c.id == value).then_some(channel);
    }
    config_channel(setup, option_id, value)
}

/// The generic channel, when the agent lists this value as a config option.
///
/// Also the fallback for an agent that publishes the spec's `modes` or `models` without
/// implementing the method that sets them — a combination that exists in the wild, and one
/// only the agent's own "method not found" can reveal.
pub fn config_channel(setup: &Value, option_id: &str, value: &str) -> Option<Channel> {
    choices(setup, option_id)
        .iter()
        .any(|c| c.id == value)
        .then_some(Channel::Config)
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

/// The spec's own mode list, or `None` when the agent does not publish one. Distinct from an
/// empty list, which is an agent saying it has no modes at all.
fn spec_modes(setup: &Value) -> Option<Vec<Choice>> {
    let listed = setup.get("modes")?.get("availableModes")?.as_array()?;
    Some(listed.iter().map(choice_from_mode).collect())
}

/// The spec's own model list. Claude reports `models: null` and a full `configOptions` entry
/// instead, which is why absence has to fall through rather than mean "no models".
fn spec_models(setup: &Value) -> Option<Vec<Choice>> {
    let listed = setup.get("models")?.get("availableModels")?.as_array()?;
    Some(listed.iter().map(choice_from_model).collect())
}

/// Permission modes: the spec's `modes.availableModes` first, falling back to the `mode`
/// config option for agents that only expose it that way.
pub fn mode_ids(setup: &Value) -> Vec<Choice> {
    spec_modes(setup).unwrap_or_else(|| choices(setup, MODE))
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

/// Models: the spec's `models.availableModels` first, else the `model` config option.
pub fn models(setup: &Value) -> Vec<Choice> {
    spec_models(setup).unwrap_or_else(|| choices(setup, MODEL))
}

/// The model the agent says it is on.
pub fn current_model(setup: &Value) -> Option<String> {
    setup
        .get("models")
        .and_then(|m| m.get("currentModelId"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| current(setup, MODEL))
}

/// What the agent currently has selected for `option_id`, whichever way it publishes it.
/// This is what makes a re-sync a no-op when nothing moved.
pub fn selected(setup: &Value, option_id: &str) -> Option<String> {
    match option_id {
        MODE => current_mode(setup),
        MODEL => current_model(setup),
        other => current(setup, other),
    }
}

/// Record a choice the agent accepted on a channel whose reply carries no state.
/// `session/set_mode` and `session/set_model` answer with nothing at all, so without this the
/// next sync would compare against a stale `currentModeId` and push the same value again.
pub fn note_current(setup: &mut Value, channel: Channel, value: &str) {
    let (group, key) = match channel {
        Channel::Mode => ("modes", "currentModeId"),
        Channel::Model => ("models", "currentModelId"),
        Channel::Config => return,
    };
    let Some(object) = setup.as_object_mut() else {
        return;
    };
    let entry = object.entry(group.to_string()).or_insert_with(|| json!({}));
    // An agent that published `modes: null` is not on this channel in the first place, so a
    // non-object here means the caller picked the wrong one; leave it rather than clobber it.
    if let Some(group) = entry.as_object_mut() {
        group.insert(key.to_string(), Value::String(value.to_string()));
    }
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
