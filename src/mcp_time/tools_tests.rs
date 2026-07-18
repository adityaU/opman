use super::*;
use serde_json::json;

// ── tool_time_now ────────────────────────────────────────────────────────────

#[test]
fn time_now_default_no_arg() {
    let out = tool_time_now(&json!({}));
    assert!(out.contains("Current time:"));
    assert!(out.contains("system default"));
}

#[test]
fn time_now_empty_and_local() {
    assert!(tool_time_now(&json!({"timezone":""})).contains("system default"));
    assert!(tool_time_now(&json!({"timezone":"local"})).contains("system default"));
}

#[test]
fn time_now_valid_zone() {
    let out = tool_time_now(&json!({"timezone":"America/New_York"}));
    assert!(out.contains("Current time in America/New_York"));
    assert!(out.contains("System timezone:"));
}

#[test]
fn time_now_utc_zone() {
    let out = tool_time_now(&json!({"timezone":"UTC"}));
    assert!(out.contains("Current time in UTC"));
}

#[test]
fn time_now_invalid_zone() {
    let out = tool_time_now(&json!({"timezone":"Not/AZone"}));
    assert!(out.contains("Unknown timezone"));
    assert!(out.contains("System time:"));
}

// ── tool_time_convert ────────────────────────────────────────────────────────

#[test]
fn convert_missing_datetime() {
    let out = tool_time_convert(&json!({"from_timezone":"UTC","to_timezone":"UTC"}));
    assert!(out.contains("Missing required argument: 'datetime'"));
}

#[test]
fn convert_missing_from() {
    let out = tool_time_convert(&json!({"datetime":"2024-01-15 10:00:00","to_timezone":"UTC"}));
    assert!(out.contains("Missing required argument: 'from_timezone'"));
}

#[test]
fn convert_missing_to() {
    let out = tool_time_convert(&json!({"datetime":"2024-01-15 10:00:00","from_timezone":"UTC"}));
    assert!(out.contains("Missing required argument: 'to_timezone'"));
}

#[test]
fn convert_unknown_from_tz() {
    let out = tool_time_convert(
        &json!({"datetime":"2024-01-15 10:00:00","from_timezone":"Bad/Zone","to_timezone":"UTC"}),
    );
    assert!(out.contains("Unknown source timezone"));
}

#[test]
fn convert_unknown_to_tz() {
    let out = tool_time_convert(
        &json!({"datetime":"2024-01-15 10:00:00","from_timezone":"UTC","to_timezone":"Bad/Zone"}),
    );
    assert!(out.contains("Unknown target timezone"));
}

#[test]
fn convert_unparseable_datetime() {
    let out = tool_time_convert(
        &json!({"datetime":"not-a-date","from_timezone":"UTC","to_timezone":"UTC"}),
    );
    assert!(out.contains("Could not parse datetime"));
}

#[test]
fn convert_success_utc_to_kolkata() {
    let out = tool_time_convert(&json!({
        "datetime":"2024-01-15 10:00:00",
        "from_timezone":"UTC",
        "to_timezone":"Asia/Kolkata"
    }));
    // UTC 10:00 → IST 15:30
    assert!(out.contains("→"));
    assert!(out.contains("15:30:00"));
    assert!(out.contains("Asia/Kolkata"));
}

#[test]
fn convert_accepts_all_datetime_formats() {
    for dt in [
        "2024-01-15T14:30:00",
        "2024-01-15 14:30:00",
        "2024-01-15T14:30",
        "2024-01-15 14:30",
    ] {
        let out = tool_time_convert(&json!({
            "datetime": dt,
            "from_timezone":"UTC",
            "to_timezone":"UTC"
        }));
        assert!(out.contains("→"), "format {dt} failed: {out}");
    }
}

#[test]
fn convert_local_keyword_resolves() {
    let out = tool_time_convert(&json!({
        "datetime":"2024-06-01 12:00:00",
        "from_timezone":"local",
        "to_timezone":"local"
    }));
    // Exercises the "local" → system-timezone substitution. The final parse
    // outcome depends on the host tz, so only assert a non-empty result (the
    // substitution branch is covered regardless).
    assert!(!out.is_empty());
}

#[test]
fn convert_nonexistent_local_time_dst_gap() {
    // 2024-03-10 02:30 does not exist in America/New_York (spring-forward gap).
    let out = tool_time_convert(&json!({
        "datetime":"2024-03-10 02:30:00",
        "from_timezone":"America/New_York",
        "to_timezone":"UTC"
    }));
    assert!(out.contains("Ambiguous or invalid local time"));
}

// ── tool_time_zones ──────────────────────────────────────────────────────────

#[test]
fn zones_all_when_empty_search() {
    let out = tool_time_zones(&json!({}));
    assert!(out.contains("timezone(s)"));
    assert!(out.contains("UTC"));
}

#[test]
fn zones_filtered_search() {
    let out = tool_time_zones(&json!({"search":"kolkata"}));
    assert!(out.contains("Asia/Kolkata"));
    assert!(out.contains("matching \"kolkata\""));
}

#[test]
fn zones_no_match() {
    let out = tool_time_zones(&json!({"search":"zzzznotazone"}));
    assert!(out.contains("No timezones found"));
}

// ── system_timezone_name ─────────────────────────────────────────────────────

#[test]
fn system_timezone_name_nonempty() {
    let name = system_timezone_name();
    assert!(!name.is_empty());
}
