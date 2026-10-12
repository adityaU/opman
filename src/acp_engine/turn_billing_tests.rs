use super::*;

#[test]
fn a_key_in_the_environment_is_named_as_the_cause() {
    let hint = billing_hint("Internal error: Credit balance is too low", true).expect("a hint");
    assert!(hint.contains("ANTHROPIC_API_KEY is set"), "{hint}");
    assert!(hint.contains("claude /login"));
}

#[test]
fn without_a_key_in_the_environment_claude_settings_are_named() {
    let hint = billing_hint("Credit balance is too low", false).expect("a hint");
    assert!(hint.contains("primaryApiKey"), "{hint}");
}

#[test]
fn other_errors_get_no_billing_hint() {
    assert!(billing_hint("acp process exited", true).is_none());
}
