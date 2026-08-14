//! Wire shape for the worktree, operation and tag types.

use super::*;

#[test]
fn a_worktree_omits_the_fields_it_has_no_answer_for() {
    let json = serde_json::to_value(GitWorktreeEntry {
        path: "/proj/wt".into(),
        relative: None,
        branch: None,
        head: "abc1234".into(),
        main: false,
        current: false,
        locked: false,
        prunable: None,
    })
    .expect("serialises");

    assert!(json.get("relative").is_none());
    assert!(json.get("branch").is_none(), "a detached worktree has no branch");
    assert!(json.get("prunable").is_none());
    assert_eq!(json["locked"], false);
}

#[test]
fn operation_kinds_serialise_in_snake_case() {
    for (kind, expected) in [
        (GitOperationKind::Merge, "merge"),
        (GitOperationKind::Rebase, "rebase"),
        (GitOperationKind::CherryPick, "cherry_pick"),
        (GitOperationKind::Revert, "revert"),
        (GitOperationKind::Bisect, "bisect"),
    ] {
        assert_eq!(serde_json::to_value(kind).expect("serialises"), expected);
    }
}

#[test]
fn a_clean_repository_reports_no_operation() {
    let json = serde_json::to_value(GitOperationResponse {
        kind: None,
        conflicted: Vec::new(),
        step: None,
        total: None,
        onto: None,
    })
    .expect("serialises");

    assert!(json.get("kind").is_none());
    assert_eq!(json["conflicted"], serde_json::json!([]));
}

#[test]
fn operation_actions_deserialise_from_their_wire_names() {
    for (wire, expected) in [
        ("\"continue\"", GitOperationAction::Continue),
        ("\"abort\"", GitOperationAction::Abort),
        ("\"skip\"", GitOperationAction::Skip),
    ] {
        let parsed: GitOperationAction = serde_json::from_str(wire).expect("deserialises");
        assert_eq!(parsed, expected);
    }
    assert!(serde_json::from_str::<GitOperationAction>("\"finish\"").is_err());
}

#[test]
fn reset_modes_map_to_their_git_flags() {
    for (mode, flag) in [
        (GitResetMode::Soft, "--soft"),
        (GitResetMode::Mixed, "--mixed"),
        (GitResetMode::Hard, "--hard"),
    ] {
        assert_eq!(mode.flag(), flag);
    }
}

#[test]
fn reset_mode_has_no_default() {
    // Losing work must be named explicitly in the request, never fallen into.
    assert!(serde_json::from_str::<GitResetRequest>(r#"{"target":"HEAD~1"}"#).is_err());
    let request: GitResetRequest =
        serde_json::from_str(r#"{"target":"HEAD~1","mode":"hard"}"#).expect("deserialises");
    assert_eq!(request.mode, GitResetMode::Hard);
}

#[test]
fn worktree_add_defaults_to_checking_out_an_existing_branch() {
    let request: GitWorktreeAddRequest =
        serde_json::from_str(r#"{"path":"wt","branch":"feature"}"#).expect("deserialises");
    assert!(!request.create);
    assert_eq!(request.start_point, None);
    assert_eq!(request.repo, "");
}

#[test]
fn worktree_remove_force_is_opt_in() {
    let request: GitWorktreeRemoveRequest =
        serde_json::from_str(r#"{"path":"wt"}"#).expect("deserialises");
    assert!(!request.force);
}

#[test]
fn camel_case_reaches_the_wire_for_multi_word_fields() {
    let request: GitWorktreeAddRequest =
        serde_json::from_str(r#"{"path":"wt","branch":"f","create":true,"startPoint":"main"}"#)
            .expect("deserialises");
    assert_eq!(request.start_point.as_deref(), Some("main"));

    let json = serde_json::to_value(GitBlameLine {
        hash: "abc".into(),
        author: "A".into(),
        date: "2026-08-14".into(),
        summary: "s".into(),
        line: 1,
        content: "code".into(),
    })
    .expect("serialises");
    assert_eq!(json["line"], 1);
}

#[test]
fn a_merge_request_defaults_to_letting_git_fast_forward() {
    let request: GitMergeRequest =
        serde_json::from_str(r#"{"branch":"feature"}"#).expect("deserialises");
    assert!(!request.no_ff);
    assert!(!request.no_commit);
}
