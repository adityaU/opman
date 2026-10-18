use super::*;

fn choice(id: &str, name: &str, description: &str) -> options::Choice {
    options::Choice {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
    }
}

fn claude_models() -> Vec<options::Choice> {
    vec![
        choice("default", "Default (recommended)", "Opus 5.5"),
        choice("sonnet", "Sonnet 5.5", "Most efficient for simpler tasks"),
        choice("claude-haiku-4-5-20251001", "Haiku 4.5", "Fastest"),
        choice("plain", "Plain", "no version"),
    ]
}

#[test]
fn an_alias_is_matched_by_its_versioned_label() {
    let (label, slug) = expectation(&claude_models(), "sonnet").expect("expectation");
    assert_eq!(label, "Sonnet 5.5");
    assert!("claude-sonnet-5-5".contains(&slug));
    assert!(!"claude-fable-5-1".contains(&slug));
}

#[test]
fn default_is_matched_by_the_model_it_describes() {
    let (_, slug) = expectation(&claude_models(), "default").expect("expectation");
    assert!("claude-opus-5-5".contains(&slug), "{slug}");
}

#[test]
fn a_dated_id_still_matches() {
    let (_, slug) =
        expectation(&claude_models(), "claude-haiku-4-5-20251001").expect("expectation");
    assert!("claude-haiku-4-5-20251001".contains(&slug));
}

#[test]
fn labels_without_a_version_or_unknown_models_are_not_checked() {
    assert!(expectation(&claude_models(), "plain").is_none());
    assert!(expectation(&claude_models(), "nope").is_none());
}

#[test]
fn clearing_the_current_model_makes_the_next_sync_push_again() {
    let mut setup = json!({
        "configOptions": [{ "id": "model", "currentValue": "sonnet", "options": [] }],
        "models": { "currentModelId": "sonnet" },
    });
    options::clear_current(&mut setup, options::MODEL);
    assert_eq!(options::selected(&setup, options::MODEL), None);
}
