//! Flattening the provider tree into the list `agent_send` and `agent_start` are validated
//! against — and keeping it small enough to read.

use super::*;

/// The shapes the live runners actually emit: opencode puts efforts under `variants`,
/// claude ships none at all, and the tree carries every provider opencode has heard of
/// whether or not it is connected.
fn providers() -> Value {
    json!({
        "all": [
            { "id": "openai", "models": {
                "gpt-5.6-luna": { "id": "gpt-5.6-luna", "name": "GPT-5.6 Luna",
                                  "variants": { "low": {}, "medium": {}, "high": {} }},
            }},
            { "id": "claude", "models": {
                "haiku": { "id": "haiku", "name": "Haiku" },
                "opus[1m]": { "id": "opus[1m]", "name": "Opus (1M context)" },
            }},
            { "id": "zhipuai", "models": {
                "glm-5.2": { "id": "glm-5.2", "name": "GLM 5.2", "reasoningEfforts": ["low"] },
            }},
        ],
        "connected": ["openai", "claude"],
        "default": { "openai": "gpt-5.6-luna" },
    })
}

fn ids<'a>(catalog: &'a Catalog<'_>) -> Vec<&'a str> {
    catalog
        .models
        .iter()
        .filter_map(|model| model["id"].as_str())
        .collect()
}

/// The defect this exists for: reading only the three array spellings reported zero
/// efforts for every one of live opencode's 6,183 models, while `agent_send` went on
/// requiring one. An agent was told the rule and denied the remedy.
#[test]
fn efforts_are_read_from_the_variants_map_the_live_runners_use() {
    let providers = providers();
    let catalog = Catalog::build(&providers, None);

    let luna = catalog
        .models
        .iter()
        .find(|model| model["id"] == "gpt-5.6-luna")
        .expect("a connected model");
    assert_eq!(luna["efforts"], json!(["high", "low", "medium"]));
}

#[test]
fn the_three_array_spellings_still_work() {
    for spelling in ["reasoningEfforts", "reasoning_efforts", "efforts"] {
        let model = json!({ spelling: ["low", "high"] });
        assert_eq!(efforts_of(&model), vec!["low", "high"], "{spelling}");
    }
    assert!(efforts_of(&json!({})).is_empty());
}

/// A model with no efforts at all is not a failure — the claude runner has none, and its
/// models are still perfectly dispatchable.
#[test]
fn a_model_without_efforts_is_listed_anyway() {
    let providers = providers();
    let catalog = Catalog::build(&providers, None);

    let haiku = catalog
        .models
        .iter()
        .find(|model| model["id"] == "haiku")
        .expect("haiku is connected");
    assert_eq!(haiku["efforts"], json!([]));
    assert_eq!(haiku["name"], "Haiku");
}

/// The 11 MB reply: opencode advertises every provider it has ever heard of, and a model
/// behind one with no credentials is not a model this caller can dispatch to.
#[test]
fn only_connected_providers_are_listed_and_the_rest_are_counted() {
    let providers = providers();
    let catalog = Catalog::build(&providers, None);

    assert_eq!(ids(&catalog), vec!["gpt-5.6-luna", "haiku", "opus[1m]"]);
    assert_eq!(catalog.total, 4);
    assert_eq!(catalog.omitted, 1, "the caller must know it was narrowed");
}

/// The escape hatch, so narrowing the default never means a model is unreachable.
#[test]
fn a_filter_searches_every_provider_including_the_unconnected_ones() {
    let providers = providers();

    let by_id = Catalog::build(&providers, Some("glm"));
    assert_eq!(ids(&by_id), vec!["glm-5.2"]);

    let by_provider = Catalog::build(&providers, Some("ZHIPU"));
    assert_eq!(by_provider.models.len(), 1, "matching is case-insensitive");

    let by_partial = Catalog::build(&providers, Some("luna"));
    assert_eq!(ids(&by_partial), vec!["gpt-5.6-luna"]);
}

#[test]
fn the_union_of_efforts_is_deduplicated_in_first_seen_order() {
    let providers = json!({ "all": [
        { "id": "a", "models": { "one": { "variants": { "low": {}, "high": {} } } } },
        { "id": "b", "models": { "two": { "reasoningEfforts": ["high", "xhigh"] } } },
    ], "connected": ["a", "b"] });

    let catalog = Catalog::build(&providers, None);

    assert_eq!(catalog.efforts, vec!["high", "low", "xhigh"]);
}

/// A model without an explicit `id` still needs one — the map key is the id, and returning
/// a model the caller cannot name would be worse than omitting it.
#[test]
fn the_map_key_stands_in_for_a_missing_id_and_name() {
    let providers = json!({
        "all": [{ "id": "p", "models": { "keyed-only": {} } }],
        "connected": ["p"],
    });

    let catalog = Catalog::build(&providers, None);

    assert_eq!(catalog.models[0]["id"], "keyed-only");
    assert_eq!(catalog.models[0]["name"], "keyed-only");
}

#[test]
fn a_provider_list_that_is_not_there_yields_nothing_rather_than_failing() {
    let providers = json!({ "connected": [] });
    let catalog = Catalog::build(&providers, None);

    assert!(catalog.models.is_empty());
    assert!(catalog.efforts.is_empty());
    assert_eq!(catalog.total, 0);
}

#[test]
fn a_provider_without_an_id_or_a_model_map_is_skipped() {
    let providers = json!({ "all": [
        { "models": { "orphan": {} } },
        { "id": "empty" },
        { "id": "real", "models": { "m": {} } },
    ], "connected": ["real"] });

    let catalog = Catalog::build(&providers, None);

    assert_eq!(catalog.models.len(), 1);
    assert_eq!(catalog.models[0]["provider"], "real");
}

/// Even one connected provider can be enormous, so the cap is the last line of defence
/// against handing back a reply nothing can read.
#[test]
fn the_model_list_is_capped_and_says_how_many_it_dropped() {
    let models: serde_json::Map<String, Value> = (0..MAX_MODELS + 25)
        .map(|n| (format!("m{n}"), json!({})))
        .collect();
    let providers = json!({
        "all": [{ "id": "many", "models": models }],
        "connected": ["many"],
    });

    let catalog = Catalog::build(&providers, None);

    assert_eq!(catalog.models.len(), MAX_MODELS);
    assert_eq!(catalog.omitted, 25);
    assert_eq!(catalog.total, MAX_MODELS + 25);
}
