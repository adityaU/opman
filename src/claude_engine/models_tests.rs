//! Coverage for the shared model catalog: the fallback list stays current, the
//! default-model heuristic, and the provider payload shape both engines serve.

use super::*;

fn m(id: &str) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        display_name: id.into(),
        context_window: 200_000,
        max_output: 64_000,
    }
}

#[test]
fn default_models_are_current_generation() {
    let models = default_models();
    assert!(models.iter().any(|m| m.id == "claude-opus-5"));
    assert!(models.iter().any(|m| m.id == "claude-sonnet-5"));
    assert!(models.iter().any(|m| m.id == "claude-fable-5"));
    // The pre-Claude-5 generation must not be the only thing on offer.
    assert!(!models.iter().any(|m| m.id == "claude-opus-4-8"));
}

#[test]
fn pick_default_prefers_sonnet_then_opus_then_first() {
    // First sonnet-or-fable in list order wins — shared with the background engine.
    assert_eq!(pick_default(&default_models()), "claude-fable-5");
    assert_eq!(pick_default(&[m("claude-opus-x")]), "claude-opus-x");
    assert_eq!(pick_default(&[m("claude-haiku-z")]), "claude-haiku-z");
    assert_eq!(pick_default(&[]), "claude-sonnet-4-6");
}

#[test]
fn provider_payload_has_opencode_shape() {
    let v = provider_payload(&default_models());
    assert_eq!(v["all"][0]["id"], "anthropic");
    assert!(v["all"][0]["models"]["claude-opus-5"].is_object());
    assert_eq!(
        v["all"][0]["models"]["claude-opus-5"]["providerID"],
        "anthropic"
    );
    assert_eq!(
        v["all"][0]["models"]["claude-opus-5"]["limit"]["context"],
        1_000_000
    );
    assert_eq!(v["connected"][0], "anthropic");
    assert_eq!(v["default"]["anthropic"], "claude-fable-5");
}
