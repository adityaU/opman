use super::*;
use crate::doctor::report::Section;

fn sample() -> Report {
    Report::new(vec![Section::new(
        "Tools",
        vec![
            Check::pass("git found"),
            Check::warn("nvim missing").detail("optional"),
            Check::fail("claude crashed").detail("line one\nline two"),
        ],
    )])
}

#[test]
fn plain_output_has_markers_and_no_escape_codes() {
    let text = human(&sample(), false);
    assert!(!text.contains('\x1b'));
    assert!(text.contains("✔ git found"));
    assert!(text.contains("⚠ nvim missing"));
    assert!(text.contains("✖ claude crashed"));
}

#[test]
fn details_are_indented_under_their_line() {
    let text = human(&sample(), false);
    assert!(text.contains("  ✖ claude crashed\n      line one\n      line two\n"));
}

#[test]
fn the_summary_is_the_last_line() {
    let text = human(&sample(), false);
    assert_eq!(
        text.trim_end().lines().last(),
        Some("1 passed, 1 warnings, 1 failed")
    );
}

#[test]
fn colour_is_added_only_when_asked() {
    assert!(human(&sample(), true).contains("\x1b[31m✖\x1b[0m"));
}

#[test]
fn json_has_a_stable_shape() {
    let value = json(&sample());
    assert_eq!(value["ok"], false);
    assert_eq!(
        value["summary"],
        serde_json::json!({ "pass": 1, "warn": 1, "fail": 1 })
    );
    let checks = &value["sections"][0]["checks"];
    assert_eq!(value["sections"][0]["name"], "Tools");
    assert_eq!(
        checks[0],
        serde_json::json!({ "status": "pass", "title": "git found" })
    );
    assert_eq!(checks[2]["status"], "fail");
    assert_eq!(checks[2]["detail"], "line one\nline two");
}
