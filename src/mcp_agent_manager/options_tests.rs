//! Flattening the provider tree into the list `agent_send` and `agent_start` are validated
//! against.

use super::*;

fn providers() -> Value {
    json!({ "all": [
        { "id": "anthropic", "models": {
            "claude-opus-5": { "id": "claude-opus-5", "name": "Opus 5", "reasoningEfforts": ["low", "high"] },
        }},
        { "id": "openai", "models": {
            "gpt-5": { "name": "GPT-5", "reasoning_efforts": ["high", "xhigh"] },
            "gpt-5-mini": { "efforts": ["low"] },
        }},
    ]})
}

#[test]
fn every_provider_model_appears_once_with_its_provider() {
    let (models, _) = flatten(&providers());

    assert_eq!(models.len(), 3);
    let opus = &models[0];
    assert_eq!(opus["provider"], "anthropic");
    assert_eq!(opus["id"], "claude-opus-5");
    assert_eq!(opus["name"], "Opus 5");
}

/// A model without an explicit `id` still needs one — the map key is the id, and returning
/// a model the caller cannot name would be worse than omitting it.
#[test]
fn the_map_key_stands_in_for_a_missing_id_and_name() {
    let (models, _) = flatten(&providers());

    let mini = models
        .iter()
        .find(|model| model["id"] == "gpt-5-mini")
        .expect("keyed models are still listed");
    assert_eq!(mini["name"], "gpt-5-mini");
}

/// All three spellings, because the runners disagree and a model with efforts must not be
/// reported as having none.
#[test]
fn efforts_are_read_under_any_of_the_three_spellings() {
    let (models, _) = flatten(&providers());

    let efforts_for = |id: &str| {
        models
            .iter()
            .find(|model| model["id"] == id)
            .map(|model| model["efforts"].clone())
            .unwrap_or(Value::Null)
    };
    assert_eq!(efforts_for("claude-opus-5"), json!(["low", "high"]));
    assert_eq!(efforts_for("gpt-5"), json!(["high", "xhigh"]));
    assert_eq!(efforts_for("gpt-5-mini"), json!(["low"]));
}

#[test]
fn the_union_of_efforts_is_deduplicated_in_first_seen_order() {
    let providers = providers();
    let (_, efforts) = flatten(&providers);

    assert_eq!(efforts, vec!["low", "high", "xhigh"]);
}

#[test]
fn a_provider_list_that_is_not_there_yields_nothing_rather_than_failing() {
    let without_all = json!({ "connected": [] });
    let (models, efforts) = flatten(&without_all);

    assert!(models.is_empty());
    assert!(efforts.is_empty());
}

#[test]
fn a_provider_without_an_id_or_a_model_map_is_skipped() {
    let (models, _) = flatten(&json!({ "all": [
        { "models": { "orphan": {} } },
        { "id": "empty" },
        { "id": "real", "models": { "m": {} } },
    ]}));

    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["provider"], "real");
}
