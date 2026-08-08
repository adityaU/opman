//! `agent_runner_options` — the answer to "what may I pass as model and effort".
//!
//! This is no longer just a convenience. `agent_send` and `agent_start` both require a
//! model and an effort, so this is the tool that makes them callable, and it flattens the
//! provider tree into a single list so the caller does not have to walk it.
//!
//! Two things the live runners forced. The raw tree is *not* passed through: opencode
//! advertises 6,183 models across every provider it has ever heard of, and returning that
//! verbatim was an 11 MB reply that no agent could read — the tool that exists to make
//! another tool callable was itself uncallable. So the answer is the models of the
//! *connected* providers, with counts saying what was left out and a `filter` for reaching
//! the rest. And efforts are read from `variants` as well, because that is the only
//! spelling either real runner actually uses.

use anyhow::Result;
use serde_json::{json, Value};

use crate::runner::{RunnerKind, RunnerRegistry};

/// How many models to return before the caller has to narrow with `filter`. Comfortably
/// above any one provider's catalogue, far below the full cross-provider list.
const MAX_MODELS: usize = 200;

pub(super) async fn runner_options(
    registry: &RunnerRegistry,
    kind: RunnerKind,
    directory: &str,
    filter: Option<&str>,
) -> Result<Value> {
    let (providers, agents) = tokio::join!(
        registry.providers(kind.clone(), directory),
        registry.agents(kind.clone(), directory),
    );
    let providers = providers?;
    let agents = agents?;
    let filter = filter.map(str::trim).filter(|text| !text.is_empty());
    let catalog = Catalog::build(&providers, filter);
    Ok(json!({
        "runner": kind,
        "models": catalog.models,
        "efforts": catalog.efforts,
        "total_models": catalog.total,
        "omitted_models": catalog.omitted,
        "connected": providers.get("connected").cloned().unwrap_or(json!([])),
        "default": providers.get("default").cloned().unwrap_or(json!({})),
        "permission_modes": providers.get("permissionModes").cloned().unwrap_or(json!([])),
        "agents": agents,
    }))
}

/// The flattened catalogue, and what it had to leave behind.
struct Catalog<'a> {
    models: Vec<Value>,
    efforts: Vec<&'a str>,
    total: usize,
    omitted: usize,
}

impl<'a> Catalog<'a> {
    /// Every model the caller could plausibly name, and the union of the efforts they
    /// accept.
    ///
    /// Unconnected providers are skipped unless a `filter` asks for them by name: a model
    /// behind a provider with no credentials is not a model this caller can dispatch to,
    /// and listing thousands of them is what made the reply unreadable.
    fn build(providers: &'a Value, filter: Option<&str>) -> Self {
        let connected = connected_ids(providers);
        let filter = filter.map(str::to_ascii_lowercase);
        let mut catalog = Self {
            models: Vec::new(),
            efforts: Vec::new(),
            total: 0,
            omitted: 0,
        };
        let Some(list) = providers.get("all").and_then(Value::as_array) else {
            return catalog;
        };
        for provider in list {
            let Some(provider_id) = provider.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(entries) = provider.get("models").and_then(Value::as_object) else {
                continue;
            };
            let reachable = filter.is_some() || connected.contains(&provider_id);
            for (model_id, model) in entries {
                catalog.total += 1;
                let id = model.get("id").and_then(Value::as_str).unwrap_or(model_id);
                if !reachable || !matches(filter.as_deref(), provider_id, id) {
                    catalog.omitted += 1;
                    continue;
                }
                catalog.push(provider_id, id, model, model_id);
            }
        }
        catalog
    }

    fn push(&mut self, provider: &str, id: &str, model: &'a Value, key: &str) {
        if self.models.len() >= MAX_MODELS {
            self.omitted += 1;
            return;
        }
        let efforts = efforts_of(model);
        for effort in &efforts {
            if !self.efforts.contains(effort) {
                self.efforts.push(effort);
            }
        }
        self.models.push(json!({
            "provider": provider,
            "id": id,
            "name": model.get("name").and_then(Value::as_str).unwrap_or(key),
            "efforts": efforts,
        }));
    }
}

/// A model the caller asked for, matched on either half of `provider/id`.
fn matches(filter: Option<&str>, provider: &str, id: &str) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    id.to_ascii_lowercase().contains(filter) || provider.to_ascii_lowercase().contains(filter)
}

fn connected_ids(providers: &Value) -> Vec<&str> {
    providers
        .get("connected")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// The runners disagree on the spelling, so accept all four rather than reporting no
/// efforts for a model that plainly has them.
///
/// `variants` is the one that matters in practice: it is a *map* keyed by effort name,
/// and it is what live opencode emits for every reasoning model it serves. Reading only
/// the three array spellings reported zero efforts for all 6,183 of them, while
/// `agent_send` went on requiring one.
fn efforts_of(model: &Value) -> Vec<&str> {
    if let Some(variants) = model.get("variants").and_then(Value::as_object) {
        return variants.keys().map(String::as_str).collect();
    }
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
