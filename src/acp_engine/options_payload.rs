//! The engine picker's catalogue, in the opencode `/provider` shape every runner emits.

use serde_json::{json, Value};

use super::Choice;

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
    effort_choices: &[Choice],
) -> Value {
    let efforts: Vec<&str> = effort_choices
        .iter()
        .map(|choice| choice.id.as_str())
        .collect();
    // `default` stays listed under the agent's own label ("Default (recommended)"): it is
    // what a session nobody configured actually runs. Hiding it left the picker showing some
    // other model for such a session while the agent quietly ran its default.
    let entries: serde_json::Map<String, Value> = models
        .iter()
        .map(|m| {
            let entry = json!({
                "id": m.id,
                "providerID": agent_id,
                "name": if m.name.is_empty() { &m.id } else { &m.name },
                "description": m.description,
                "limit": { "context": 0, "output": 0 },
                "reasoningEfforts": efforts,
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
    // The model a fresh session starts on, when the list names it.
    let default = current_model
        .filter(|id| !id.is_empty() && entries.contains_key(*id))
        .map(|id| json!({ agent_id: id }))
        .unwrap_or_else(|| json!({}));
    json!({
        "all": [{ "id": agent_id, "name": display_name, "models": entries }],
        "connected": [agent_id],
        "default": default,
        "permissionModes": modes,
    })
}
