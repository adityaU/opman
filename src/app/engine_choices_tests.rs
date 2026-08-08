use super::*;

#[test]
fn a_choice_cannot_hold_nothing() {
    assert!(Choice::new("").is_none());
    assert!(Choice::new("   ").is_none());
    assert_eq!(Choice::new(" opus ").as_deref(), Some("opus"));
}

#[test]
fn empty_strings_from_the_wire_read_as_never_chosen() {
    // Every runner spells "not set" as the empty string somewhere. Reading one as a
    // selection is what put an unmatchable value in the composer.
    let choices: EngineChoices =
        serde_json::from_str(r#"{"model":"","agent":"plan","effort":null}"#).unwrap();
    assert_eq!(choices.model, None);
    assert_eq!(choices.agent.as_deref(), Some("plan"));
    assert_eq!(choices.effort, None);
    assert_eq!(choices.permission_mode, None);
}

#[test]
fn an_absent_object_is_a_session_that_was_never_configured() {
    let choices: EngineChoices = serde_json::from_str("{}").unwrap();
    assert!(choices.is_empty());
}

#[test]
fn permission_mode_keeps_its_wire_name() {
    let choices = EngineChoices::from_parts(None, None, None, Some("plan"));
    let encoded = serde_json::to_string(&choices).unwrap();
    assert!(encoded.contains(r#""permissionMode":"plan""#), "{encoded}");

    let decoded: EngineChoices = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, choices);
}

#[test]
fn from_parts_drops_the_fields_an_engine_has_no_answer_for() {
    let choices = EngineChoices::from_parts(Some("opus"), Some(""), None, Some("  "));
    assert_eq!(choices.model.as_deref(), Some("opus"));
    assert_eq!(choices.agent, None);
    assert_eq!(choices.effort, None);
    assert_eq!(choices.permission_mode, None);
}
