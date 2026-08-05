//! Shared Anthropic model catalog for both embedded engines.
//!
//! The `--claude` (background-agent) and `--claude-p` (streaming print) engines expose
//! the same opencode `/provider` contract, so the fallback list, the default-model
//! heuristic, and the payload shape live here rather than being duplicated — a stale
//! copy in one engine is exactly the drift this module exists to prevent.

use serde_json::{json, Value};

use super::claude_cli::ModelInfo;

/// Fallback catalog used when dynamic discovery (`claude_cli::fetch_models_via_cli`)
/// is unavailable — the CLI is missing, offline, or the probe turn hasn't landed yet.
pub fn default_models() -> Vec<ModelInfo> {
    let m = |id: &str, name: &str, context: u64, max_output: u64| ModelInfo {
        id: id.into(),
        display_name: name.into(),
        context_window: context,
        max_output,
    };
    vec![
        m("claude-fable-5", "Claude Fable 5", 1_000_000, 128_000),
        m("claude-opus-5", "Claude Opus 5", 1_000_000, 128_000),
        m("claude-sonnet-5", "Claude Sonnet 5", 1_000_000, 128_000),
        m("claude-sonnet-4-6", "Claude Sonnet 4.6", 1_000_000, 64_000),
        m(
            "claude-haiku-4-5-20251001",
            "Claude Haiku 4.5",
            200_000,
            32_000,
        ),
    ]
}

/// Pick the best default from a list: prefer a sonnet or fable, then opus, then first.
pub fn pick_default(models: &[ModelInfo]) -> &str {
    models
        .iter()
        .find(|m| m.id.contains("sonnet") || m.id.contains("fable"))
        .or_else(|| models.iter().find(|m| m.id.contains("opus")))
        .or_else(|| models.first())
        .map(|m| m.id.as_str())
        .unwrap_or("claude-sonnet-4-6")
}

/// Build the opencode `{ all, connected, default }` provider payload for `models`.
pub fn provider_payload(models: &[ModelInfo]) -> Value {
    let default_id = pick_default(models).to_string();
    let models_map: serde_json::Map<String, Value> = models
        .iter()
        .map(|m| {
            let v = json!({
                "id": m.id,
                "providerID": "anthropic",
                "name": m.display_name,
                "limit": { "context": m.context_window, "output": m.max_output },
            });
            (m.id.clone(), v)
        })
        .collect();

    json!({
        "all": [{ "id": "anthropic", "name": "Anthropic", "models": models_map }],
        "connected": ["anthropic"],
        "default": { "anthropic": default_id },
    })
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod models_tests;
