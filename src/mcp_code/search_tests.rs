use super::*;
use crate::mcp_code::catalog::tests_support::sample;

#[test]
fn finds_by_name_and_description() {
    let hits = search(&sample(), "add numbers");
    let first = &hits[0];
    assert_eq!(first["path"], "tools.fake.add");
    assert!(first["signature"]
        .as_str()
        .unwrap_or("")
        .contains("await tools.fake.add("));
}

#[test]
fn no_match_is_an_empty_list() {
    assert_eq!(search(&sample(), "zzzz"), serde_json::json!([]));
    assert_eq!(search(&sample(), ""), serde_json::json!([]));
}

#[test]
fn name_hits_beat_description_hits() {
    // "image" is in one tool's name and also in its description; "picture" only in one.
    let hits = search(&sample(), "image");
    assert_eq!(hits[0]["path"], "tools.fake.image");
}

#[test]
fn hits_are_capped() {
    let hits = search(&sample(), "a e i o u");
    assert!(hits.as_array().map_or(0, Vec::len) <= 8);
}
