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

/// The permission modes this engine accepts, with what each one means.
///
/// Published on `/provider` for the same reason the ACP engines publish theirs: the mode
/// names are this runner's vocabulary, and anything that has to name one — the engine
/// picker, the `/permission-mode` command, the agent-manager MCP — otherwise needs its own
/// copy of the list, which is how one of them ends up offering a mode the engine will
/// reject.
pub const PERMISSION_MODES: [(&str, &str, &str); 6] = [
    ("default", "Manual", "Prompts before anything consequential"),
    (
        "acceptEdits",
        "Accept Edits",
        "Auto-accept file edits, prompt for the rest",
    ),
    (
        "auto",
        "Auto",
        "Let a classifier answer the permission prompts",
    ),
    (
        "bypassPermissions",
        "Bypass Permissions",
        "Run everything without prompting",
    ),
    (
        "dontAsk",
        "Don't Ask",
        "Never prompt; deny whatever is not pre-approved",
    ),
    ("plan", "Plan Mode", "Plan only, execute no tools"),
];

/// Whether `mode` is one this engine accepts, in its canonical spelling.
pub fn permission_mode(mode: &str) -> Option<&'static str> {
    PERMISSION_MODES
        .iter()
        .find(|(id, _, _)| id.eq_ignore_ascii_case(mode))
        .map(|(id, _, _)| *id)
}

/// Build the opencode `{ all, connected, default, permissionModes }` provider payload.
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

    let modes: Vec<Value> = PERMISSION_MODES
        .iter()
        .map(|(value, label, description)| json!({
            "value": value,
            "label": label,
            "description": description,
        }))
        .collect();

    json!({
        "all": [{ "id": "anthropic", "name": "Anthropic", "models": models_map }],
        "connected": ["anthropic"],
        "default": { "anthropic": default_id },
        "permissionModes": modes,
    })
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod models_tests;
