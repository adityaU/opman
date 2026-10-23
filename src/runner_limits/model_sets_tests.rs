//! Sets follow the active model and keep fallback rules for sessions without a match.

use chrono::Utc;
use serde_json::json;

use super::*;
use crate::runner::RunnerKind;
use crate::runner_limits::choose::{choose_for_model, Accounts, Level};

fn settings() -> Result<LimitSettings> {
    Ok(serde_json::from_value(json!({
        "pairs": [{"runner":"opencode"}],
        "modelSets": [
            {"id":"large", "name":"Large", "members":[
                {"runner":"claude", "model":{"providerID":"anthropic","modelID":"opus"}},
                {"runner":"codex", "model":{"providerID":"codex","modelID":"sol"},"permission":"read-only"}
            ]},
            {"id":"small", "name":"Small", "members":[
                {"runner":"claude", "model":{"providerID":"anthropic","modelID":"sonnet"}},
                {"runner":"codex", "model":{"providerID":"codex","modelID":"luna"}}
            ]}
        ]
    }))?)
}

fn model(provider: &str, id: &str) -> ModelRef {
    ModelRef {
        provider_id: provider.into(),
        model_id: id.into(),
    }
}

#[test]
fn selecting_a_model_uses_its_set_and_preserves_target_options() -> Result<()> {
    let settings = settings()?;
    let available = [
        RunnerKind::Claude,
        RunnerKind::Acp("codex".into()),
        RunnerKind::Opencode,
    ];
    for (source, target) in [("opus", "sol"), ("sonnet", "luna")] {
        let picked = choose_for_model(
            &settings,
            &RunnerKind::Claude,
            Some(&model("anthropic", source)),
            Level::Near,
            &available,
            &Accounts::default(),
            Utc::now(),
        );
        let Some((runner, pair)) = picked else {
            anyhow::bail!("missing target");
        };
        assert_eq!(runner, RunnerKind::Acp("codex".into()));
        assert_eq!(
            pair.model.as_ref().map(|model| model.model_id.as_str()),
            Some(target)
        );
        if source == "opus" {
            assert_eq!(pair.permission.as_deref(), Some("read-only"));
        }
    }
    Ok(())
}

#[test]
fn a_matching_set_does_not_fall_through_to_an_unrelated_model() -> Result<()> {
    let settings = settings()?;
    let picked = choose_for_model(
        &settings,
        &RunnerKind::Claude,
        Some(&model("anthropic", "opus")),
        Level::Out,
        &[RunnerKind::Opencode],
        &Accounts::default(),
        Utc::now(),
    );
    assert!(picked.is_none());
    Ok(())
}

#[test]
fn missing_or_unmatched_models_keep_legacy_fallbacks() -> Result<()> {
    let settings = settings()?;
    for model in [
        None,
        Some(model("anthropic", "haiku")),
        Some(model("other", "opus")),
    ] {
        assert_eq!(
            settings.candidates("claude", model.as_ref()),
            &settings.pairs
        );
    }
    assert_eq!(
        settings.candidates("claude", Some(&model("", "opus"))),
        &settings.model_sets[0].members
    );
    Ok(())
}

#[test]
fn invalid_sets_are_rejected_before_the_store_changes() -> Result<()> {
    let valid = settings()?;
    valid.validate()?;
    let store = super::super::settings::SettingsStore::open(None);
    let mut invalid_cases = vec![];
    let mut missing_model = valid.clone();
    missing_model.model_sets[0].members[0].model = None;
    invalid_cases.push(missing_model);
    let mut duplicate = valid.clone();
    duplicate.model_sets.push(valid.model_sets[0].clone());
    invalid_cases.push(duplicate);
    let mut one_runner = valid.clone();
    one_runner.model_sets[0].members[1].runner = "claude".into();
    invalid_cases.push(one_runner);
    let mut repeated_member = valid.clone();
    repeated_member.model_sets[1].members[0] = valid.model_sets[0].members[0].clone();
    invalid_cases.push(repeated_member);
    for invalid in invalid_cases {
        assert!(store.set(invalid).is_err());
        assert!(store.get().model_sets.is_empty());
    }
    Ok(())
}

#[test]
fn sets_round_trip_through_the_settings_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("runner-limits.json");
    let next = settings()?;
    super::super::settings::SettingsStore::open(Some(path.clone())).set(next.clone())?;
    assert_eq!(
        *super::super::settings::SettingsStore::open(Some(path)).get(),
        next
    );
    Ok(())
}

#[test]
fn sets_work_in_both_directions() -> Result<()> {
    let settings = settings()?;
    let picked = choose_for_model(
        &settings,
        &RunnerKind::Acp("codex".into()),
        Some(&model("codex", "luna")),
        Level::Near,
        &[RunnerKind::Claude],
        &Accounts::default(),
        Utc::now(),
    );
    assert_eq!(
        picked.map(|(_, pair)| pair),
        Some(&settings.model_sets[1].members[0])
    );
    Ok(())
}

#[test]
fn bare_model_ids_never_guess_between_two_providers() -> Result<()> {
    let mut settings = settings()?;
    settings.model_sets[1].members[0].model = Some(model("other", "opus"));
    settings.validate()?;
    assert_eq!(
        settings.candidates("claude", Some(&model("", "opus"))),
        &settings.pairs
    );
    assert_eq!(
        settings.candidates("claude", Some(&model("anthropic", "opus"))),
        &settings.model_sets[0].members
    );
    Ok(())
}
