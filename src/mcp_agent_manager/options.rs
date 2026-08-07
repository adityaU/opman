//! `agent_runner_options` — the answer to "what may I pass as model and effort".
//!
//! This is no longer just a convenience. `agent_send` and `agent_start` both require a
//! model and an effort, so this is the tool that makes them callable, and it flattens the
//! provider tree into a single list so the caller does not have to walk it.

use anyhow::Result;
use serde_json::{json, Value};

use crate::runner::{RunnerKind, RunnerRegistry};

pub(super) async fn runner_options(
    registry: &RunnerRegistry,
    kind: RunnerKind,
    directory: &str,
) -> Result<Value> {
    let (providers, agents) = tokio::join!(
        registry.providers(kind.clone(), directory),
        registry.agents(kind.clone(), directory),
    );
    let providers = providers?;
    let agents = agents?;
    let (models, efforts) = flatten(&providers);
    Ok(json!({
        "runner": kind,
        "models": models,
        "efforts": efforts,
        "providers": providers,
        "agents": agents,
    }))
}

/// Every model across every provider, and the union of the efforts they accept.
fn flatten(providers: &Value) -> (Vec<Value>, Vec<&str>) {
    let mut models = Vec::new();
    let mut efforts: Vec<&str> = Vec::new();
    let Some(list) = providers.get("all").and_then(Value::as_array) else {
        return (models, efforts);
    };
    for provider in list {
        let Some(provider_id) = provider.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(entries) = provider.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_id, model) in entries {
            let model_efforts = efforts_of(model);
            for effort in &model_efforts {
                if !efforts.contains(effort) {
                    efforts.push(effort);
                }
            }
            models.push(json!({
                "provider": provider_id,
                "id": model.get("id").and_then(Value::as_str).unwrap_or(model_id),
                "name": model.get("name").and_then(Value::as_str).unwrap_or(model_id),
                "efforts": model_efforts,
            }));
        }
    }
    (models, efforts)
}

/// The runners disagree on the spelling, so accept all three rather than reporting no
/// efforts for a model that plainly has them.
fn efforts_of(model: &Value) -> Vec<&str> {
    model
        .get("reasoningEfforts")
        .or_else(|| model.get("reasoning_efforts"))
        .or_else(|| model.get("efforts"))
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod options_tests;
