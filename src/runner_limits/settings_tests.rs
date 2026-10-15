//! The settings file: what the browser sends is what is stored, and anything odd is
//! refused at the type level rather than half-saved.

use super::*;

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[test]
fn defaults_are_ask_at_five_percent_with_no_pairs() {
    let settings = LimitSettings::default();
    assert_eq!(settings.policy, LimitPolicy::Ask);
    assert_eq!(u8::from(settings.near_percent), 5);
    assert!(settings.pairs.is_empty());
}

#[test]
fn the_browser_shape_round_trips() {
    let raw = r#"{"policy":"auto","nearPercent":10,
        "pairs":[{"runner":"codex","model":{"providerID":"openai","modelID":"gpt-5.6-sol"}},
                 {"runner":"opencode","model":null}]}"#;
    let settings: LimitSettings = serde_json::from_str(raw).expect("parses");
    assert_eq!(settings.policy, LimitPolicy::Auto);
    assert_eq!(
        settings.pairs[0]
            .model
            .as_ref()
            .map(|m| m.model_id.as_str()),
        Some("gpt-5.6-sol")
    );
    assert_eq!(settings.pairs[1].model, None);
    assert_eq!(settings.pairs[0].permission, None);
    let back = serde_json::to_value(&settings).expect("serializes");
    assert_eq!(back["nearPercent"], 10);
    assert_eq!(back["pairs"][0]["model"]["providerID"], "openai");
    assert!(back["pairs"][0].get("permission").is_none());
}

#[test]
fn a_target_permission_round_trips_and_null_remains_compatible() -> Result<()> {
    let settings: LimitSettings = serde_json::from_value(serde_json::json!({
        "pairs": [
            {"runner": "codex", "permission": "read-only"},
            {"runner": "claude-code", "permission": null}
        ]
    }))?;
    assert_eq!(settings.pairs[0].permission.as_deref(), Some("read-only"));
    assert_eq!(settings.pairs[1].permission, None);
    let back = serde_json::to_value(settings)?;
    assert_eq!(back["pairs"][0]["permission"], "read-only");
    Ok(())
}

#[test]
fn a_threshold_outside_one_to_ninety_nine_is_refused() {
    for bad in [
        r#"{"nearPercent":0}"#,
        r#"{"nearPercent":100}"#,
        r#"{"policy":"sometimes"}"#,
    ] {
        assert!(serde_json::from_str::<LimitSettings>(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_save_is_read_back_by_a_new_store() {
    let dir = dir();
    let path = dir.path().join("runner-limits.json");
    let store = SettingsStore::open(Some(path.clone()));
    let next = LimitSettings {
        policy: LimitPolicy::Auto,
        near_percent: NearPercent::try_from(20).expect("valid"),
        pairs: vec![FallbackPair {
            runner: "codex".into(),
            model: None,
            permission: Some("read-only".into()),
        }],
    };
    store.set(next.clone()).expect("saves");
    assert_eq!(*store.get(), next);
    assert_eq!(*SettingsStore::open(Some(path)).get(), next);
}

#[test]
fn a_broken_file_falls_back_to_defaults() {
    let dir = dir();
    let path = dir.path().join("runner-limits.json");
    std::fs::write(&path, "{not json").expect("writes");
    assert_eq!(
        *SettingsStore::open(Some(path)).get(),
        LimitSettings::default()
    );
}
