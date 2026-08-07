//! The required halves, and the body they produce.

use super::*;

fn ok(model: &str, effort: &str) -> Dispatch {
    Dispatch::parse(Some(model), Some(effort), None).expect("both halves given")
}

#[test]
fn a_missing_model_says_which_tool_answers_the_question() {
    let error = Dispatch::parse(None, Some("high"), None).expect_err("model is required");

    let text = format!("{error}");
    assert!(text.contains("'model' is required"), "{text}");
    assert!(text.contains("agent_runner_options"), "{text}");
}

#[test]
fn a_missing_effort_says_which_tool_answers_the_question() {
    let error = Dispatch::parse(Some("sonnet"), None, None).expect_err("effort is required");

    let text = format!("{error}");
    assert!(text.contains("'effort' is required"), "{text}");
    assert!(text.contains("agent_runner_options"), "{text}");
}

/// Whitespace is the interesting empty: JSON Schema's `required` is satisfied by `""`, so
/// the check that actually holds the contract is this one.
#[test]
fn blank_is_the_same_as_absent() {
    assert!(Dispatch::parse(Some("   "), Some("high"), None).is_err());
    assert!(Dispatch::parse(Some("sonnet"), Some("\t"), None).is_err());
}

#[test]
fn the_body_carries_both_halves_where_every_runner_reads_them() {
    let body = ok("sonnet", "high").body("hello");

    assert_eq!(body["parts"][0]["text"], "hello");
    assert_eq!(body["model"]["modelID"], "sonnet");
    // Top level, not nested under `model`: OpenCode renames it, Codex forwards it, and
    // the Claude engine turns it into `--effort`, all from here.
    assert_eq!(body["effort"], "high");
}

#[test]
fn a_provider_is_optional_and_an_empty_one_is_no_provider() {
    let named = Dispatch::parse(Some("gpt-5"), Some("medium"), Some("openai")).expect("valid");
    assert_eq!(named.body("x")["model"]["providerID"], "openai");

    let blank = Dispatch::parse(Some("gpt-5"), Some("medium"), Some("  ")).expect("valid");
    assert_eq!(blank.body("x")["model"]["providerID"], "");
}

#[test]
fn the_halves_are_trimmed_so_a_padded_argument_still_matches() {
    assert_eq!(ok(" sonnet ", " high "), ok("sonnet", "high"));
}
