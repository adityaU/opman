//! Branch-row parsing, including the fields git leaves empty.

use super::*;

fn head() -> HeadState {
    HeadState::Attached("main".to_string())
}

#[test]
fn parses_a_fully_populated_row() {
    let line = "feature/login\torigin/feature/login\t[ahead 3, behind 1]\t/tmp/wt\t2026-08-14T10:00:00+00:00\tAdd the login form";
    let info = parse(line, false, &head()).expect("row parses");

    assert_eq!(info.name, "feature/login");
    assert_eq!(info.upstream.as_deref(), Some("origin/feature/login"));
    assert_eq!((info.ahead, info.behind), (3, 1));
    assert_eq!(info.worktree.as_deref(), Some("/tmp/wt"));
    assert_eq!(info.subject, "Add the login form");
    assert!(!info.current);
    assert!(!info.remote);
}

#[test]
fn marks_the_checked_out_branch_current() {
    let line = "main\t\t\t\t2026-08-14T10:00:00+00:00\tRoot";
    let info = parse(line, false, &head()).expect("row parses");
    assert!(info.current);

    // A remote ref of the same name is never the current branch.
    let remote = parse(line, true, &head()).expect("row parses");
    assert!(!remote.current);
}

#[test]
fn absent_upstream_and_worktree_become_none() {
    let line = "solo\t\t\t\t2026-08-14T10:00:00+00:00\tOnly commit";
    let info = parse(line, false, &head()).expect("row parses");
    assert_eq!(info.upstream, None);
    assert_eq!(info.worktree, None);
    assert_eq!((info.ahead, info.behind), (0, 0));
}

#[test]
fn a_subject_containing_tabs_stays_in_one_field() {
    let line = "b\t\t\t\t2026-08-14T10:00:00+00:00\tfix:\tcolumns\tsurvive";
    let info = parse(line, false, &head()).expect("row parses");
    assert_eq!(info.subject, "fix:\tcolumns\tsurvive");
}

#[test]
fn skips_the_symbolic_remote_head() {
    let line = "origin/HEAD\t\t\t\t2026-08-14T10:00:00+00:00\t";
    assert!(parse(line, true, &head()).is_none());
}

#[test]
fn skips_blank_rows() {
    assert!(parse("", false, &head()).is_none());
    assert!(parse("\t\t\t\t\t", false, &head()).is_none());
}

#[test]
fn track_parsing_covers_every_shape_git_emits() {
    assert_eq!(parse_track(""), (0, 0));
    assert_eq!(parse_track("[ahead 3]"), (3, 0));
    assert_eq!(parse_track("[behind 2]"), (0, 2));
    assert_eq!(parse_track("[ahead 3, behind 2]"), (3, 2));
    assert_eq!(parse_track("[gone]"), (0, 0));
}

#[test]
fn head_state_reports_a_branch_only_when_attached() {
    assert_eq!(HeadState::Attached("x".into()).branch(), Some("x"));
    assert_eq!(
        HeadState::Detached {
            short: "abc1234".into()
        }
        .branch(),
        None
    );
    assert_eq!(HeadState::Unborn.branch(), None);
}
