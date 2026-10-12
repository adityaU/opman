//! `usage_update`: token counts, and the model the turn really ran on.
//!
//! Claude names the model that answered in `_meta["_claude/model"]`. That is the only
//! ground truth for "did my model switch take": a switch the agent refused or never
//! received otherwise runs the turn on its default under the chosen model's name.

use std::sync::Arc;

use serde_json::{json, Value};

use super::options;
use super::AcpEngine;

pub(super) fn apply(engine: &Arc<AcpEngine>, session_id: &str, update: &Value) {
    let used = update.get("used").and_then(Value::as_u64).unwrap_or(0);
    let size = update.get("size").and_then(Value::as_u64).unwrap_or(0);
    let cost = update
        .get("cost")
        .and_then(|c| c.get("amount"))
        .and_then(Value::as_f64);
    let tokens = json!({
        "input": used,
        "output": 0,
        "reasoning": 0,
        "context": { "used": used, "size": size },
        "cache": { "read": 0, "write": 0 },
    });
    let ran_on = update
        .pointer("/_meta/_claude~1model")
        .and_then(Value::as_str);
    let (emits, restamped) = engine.with_transcript(session_id, |t| {
        let mut out = Vec::new();
        t.set_usage(tokens, cost, &mut out);
        let restamped = ran_on.is_some_and(|model| t.note_ran_on(model, &mut out));
        (out, restamped)
    });
    super::render::broadcast(engine, session_id, emits);
    if let (true, Some(model)) = (restamped, ran_on) {
        check(engine, session_id, model);
    }
}

/// Compare the model that answered with the one the session asked for. On a mismatch, say
/// so and forget the agent's reported selection, so the next prompt pushes the model again.
fn check(engine: &Arc<AcpEngine>, session_id: &str, ran_on: &str) {
    let Some(wanted) = engine
        .get_session(session_id)
        .and_then(|session| session.model)
        .filter(|model| !model.is_empty())
    else {
        return;
    };
    let setup = engine.session_setup(session_id);
    let Some((label, expected)) = expectation(&options::models(&setup), &wanted) else {
        return;
    };
    if ran_on.contains(&expected) {
        return;
    }
    engine.forget_selected(session_id, options::MODEL);
    let text = format!(
        "Asked for {label}, but the agent answered with `{ran_on}`. opman will switch the \
         model again before the next message."
    );
    super::emit_system(engine, session_id, "warning", &text);
}

/// The model's display label and the id fragment it should appear as: "Sonnet 5.5" →
/// `sonnet-5-5`, matched inside `claude-sonnet-5-5`. `default` is described by the model it
/// stands for. `None` when the label carries no version to match on.
pub(super) fn expectation(models: &[options::Choice], wanted: &str) -> Option<(String, String)> {
    let choice = models.iter().find(|choice| choice.id == wanted)?;
    let label = match choice.id.as_str() {
        "default" => &choice.description,
        _ => &choice.name,
    };
    let slug: String = label
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let has_version = slug.chars().any(|c| c.is_ascii_digit());
    (has_version && !slug.is_empty()).then(|| (label.to_string(), slug))
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod usage_tests;
