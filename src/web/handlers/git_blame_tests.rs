//! Blame parsing, including the per-block metadata carry-forward.

use super::git_refs_tests::*;
use super::super::git_refs::{git_blame, iso_date, parse_tz};
use axum::extract::{Query, State};

#[tokio::test]
async fn blame_carries_block_metadata_forward() {
    let td = init_repo();
    let dir = td.path();
    // Alice writes a three-line block; Bob appends one line later.
    commit_as(
        dir,
        "Alice",
        "alice@example.com",
        "f.txt",
        "one\ntwo\nthree\n",
        "alice work",
    );
    commit_as(
        dir,
        "Bob",
        "bob@example.com",
        "f.txt",
        "one\ntwo\nthree\nfour\n",
        "bob work",
    );
    let state = state_for(dir);

    let body = body_json(blame(&state, "f.txt").await).await;
    let lines = body["lines"].as_array().expect("lines array");
    assert_eq!(lines.len(), 4);

    for (i, line) in lines.iter().enumerate().take(3) {
        // The header block appears once; lines 2 and 3 must not lose it.
        assert_eq!(line["author"], "Alice", "line {} lost its author", i + 1);
        assert_eq!(line["summary"], "alice work");
        assert!(line["date"].as_str().unwrap_or("").contains('T'));
        assert_eq!(line["line"], (i + 1) as u64);
    }
    assert_eq!(lines[0]["content"], "one");
    assert_eq!(lines[2]["content"], "three");
    assert_eq!(lines[3]["author"], "Bob");
    assert_eq!(lines[3]["summary"], "bob work");
    assert_eq!(lines[3]["content"], "four");
    assert_ne!(lines[0]["hash"], lines[3]["hash"]);
}

#[tokio::test]
async fn blame_of_a_missing_file_is_a_client_error() {
    let td = init_repo();
    let dir = td.path();
    commit_as(dir, "A", "a@example.com", "a.txt", "one\n", "c1");
    let state = state_for(dir);
    assert!(blame(&state, "nope.txt").await.is_err());
}

#[tokio::test]
async fn blame_rejects_an_option_shaped_path() {
    let td = init_repo();
    let state = state_for(td.path());
    assert!(blame(&state, "--help").await.is_err());
}

#[test]
fn tz_offsets_parse() {
    assert_eq!(parse_tz("+0530"), Some(19800));
    assert_eq!(parse_tz("-0800"), Some(-28800));
    assert_eq!(parse_tz("+0000"), Some(0));
    assert_eq!(parse_tz("junk"), None);
    assert_eq!(parse_tz(""), None);
}

#[test]
fn iso_date_uses_the_committer_offset() {
    assert!(iso_date(0, "+0530").starts_with("1970-01-01T05:30:00"));
    assert!(iso_date(0, "junk").starts_with("1970-01-01T00:00:00"));
}
