use super::*;

fn kind(text: &str) -> Option<LimitKind> {
    classify(text).map(|hit| hit.kind)
}

#[test]
fn claude_usage_limit_with_epoch_reset() {
    let hit = classify("Claude AI usage limit reached|1760000000").expect("limit");
    assert_eq!(hit.kind, LimitKind::Usage);
    assert_eq!(hit.resets_at.as_deref(), Some("2025-10-09T08:53:20Z"));
}

#[test]
fn epoch_in_milliseconds_is_scaled() {
    let hit = classify("Claude AI usage limit reached|1760000000000").expect("limit");
    assert_eq!(hit.resets_at.as_deref(), Some("2025-10-09T08:53:20Z"));
}

#[test]
fn claude_hit_your_limit_variants() {
    for text in [
        "You've hit your limit · resets 3pm (UTC)",
        "You’ve hit your 5-hour limit. Upgrade or wait.",
        "You have hit your weekly limit",
        "Internal error: You've reached your usage limit for Opus",
    ] {
        assert_eq!(kind(text), Some(LimitKind::Usage), "{text}");
        assert_eq!(classify(text).and_then(|h| h.resets_at), None, "{text}");
    }
}

#[test]
fn codex_usage_limit_variants() {
    assert_eq!(
        kind("You've hit your usage limit. Upgrade to Pro or try again at 3:05 PM."),
        Some(LimitKind::Usage)
    );
    assert_eq!(
        kind(r#"stream error: {"type":"usage_limit_reached","resets_in_seconds":900}"#),
        Some(LimitKind::Usage)
    );
}

#[test]
fn generic_rate_limits() {
    for text in [
        "429 Too Many Requests",
        "API Error: rate limit exceeded",
        "error: rate_limit_error",
        "Request was rate-limited, retry later",
    ] {
        assert_eq!(kind(text), Some(LimitKind::Rate), "{text}");
    }
}

#[test]
fn usage_wins_over_rate_when_both_are_named() {
    assert_eq!(
        kind("rate_limit: Claude AI usage limit reached|1760000000"),
        Some(LimitKind::Usage)
    );
}

#[test]
fn unrelated_errors_are_not_limits() {
    for text in [
        "",
        "The turn hit the model's output limit.",
        "The turn hit the agent's request limit.",
        "Credit balance is too low",
        "Overloaded. Please try again.",
        "acp process exited",
        "syntax error at line 429",
        "You've hit your stride. Nice work — no limit here",
        "Please generate a summary",
        "Context window limit reached; compacting",
    ] {
        assert_eq!(classify(text), None, "{text}");
    }
}

#[test]
fn pipe_without_timestamp_has_no_reset() {
    let hit = classify("usage limit reached | try later").expect("limit");
    assert_eq!(hit.resets_at, None);
}

#[test]
fn tag_serialises_the_wire_shape() {
    let value = tag("Claude AI usage limit reached|1760000000").expect("tag");
    assert_eq!(value["kind"], "usage");
    assert_eq!(value["resetsAt"], "2025-10-09T08:53:20Z");
    let rate = tag("429 too many requests").expect("tag");
    assert_eq!(rate["kind"], "rate");
    assert!(rate.get("resetsAt").is_none());
    assert!(tag("everything is fine").is_none());
}
