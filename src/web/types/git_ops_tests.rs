//! Serialisation shape of the mutation outcome.
//!
//! The frontend branches on `ok` and `failure`, so the exact JSON matters as
//! much as the Rust types do.

use super::*;
use crate::web::git::GitOutput;

fn output(stdout: &str, stderr: &str) -> GitOutput {
    GitOutput {
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

#[test]
fn success_omits_the_failure_fields() {
    let json = serde_json::to_value(GitActionResponse::succeeded(&output(
        "",
        "Switched to branch 'main'",
    )))
    .expect("serialises");

    assert_eq!(json["ok"], true);
    assert_eq!(json["message"], "Switched to branch 'main'");
    assert!(json.get("failure").is_none(), "no failure on success");
    assert!(json.get("hint").is_none(), "no hint on success");
}

#[test]
fn a_refusal_carries_a_code_and_a_recovery() {
    let refusal = GitRefusal {
        failure: GitFailure::DirtyTree,
        detail: "error: Your local changes would be overwritten".to_string(),
    };
    let json = serde_json::to_value(GitActionResponse::refused(refusal)).expect("serialises");

    assert_eq!(json["ok"], false);
    assert_eq!(json["failure"], "dirty_tree");
    assert!(json["hint"].as_str().is_some_and(|h| h.contains("stash")));
    assert!(json["message"]
        .as_str()
        .is_some_and(|m| m.contains("local changes")));
}

#[test]
fn every_failure_code_serialises_in_snake_case() {
    for (failure, expected) in [
        (GitFailure::AuthRequired, "auth_required"),
        (GitFailure::DirtyTree, "dirty_tree"),
        (GitFailure::Conflict, "conflict"),
        (GitFailure::NotFound, "not_found"),
        (GitFailure::Rejected, "rejected"),
        (GitFailure::Locked, "locked"),
        (GitFailure::Failed, "failed"),
    ] {
        let json = serde_json::to_value(failure).expect("serialises");
        assert_eq!(json, expected);
    }
}

#[test]
fn blocked_reports_our_own_reason_with_the_matching_hint() {
    let response = GitActionResponse::blocked(GitFailure::NotFound, "No remote configured");
    assert!(!response.ok);
    assert_eq!(response.failure, Some(GitFailure::NotFound));
    assert_eq!(response.hint, Some(GitFailure::NotFound.hint()));
    assert_eq!(response.message, "No remote configured");
}

#[test]
fn from_git_result_maps_both_arms() {
    let ok: GitResult = Ok(output("", "done"));
    assert!(GitActionResponse::from(ok).ok);

    let refused: GitResult = Err(GitRefusal {
        failure: GitFailure::Conflict,
        detail: "CONFLICT".to_string(),
    });
    let response = GitActionResponse::from(refused);
    assert!(!response.ok);
    assert_eq!(response.failure, Some(GitFailure::Conflict));
}

#[test]
fn the_commit_response_flattens_its_action() {
    let json = serde_json::to_value(GitCommitResponse {
        action: GitActionResponse::succeeded(&output("", "1 file changed")),
        hash: Some("abc1234".to_string()),
    })
    .expect("serialises");

    // Flattened, not nested — the client reads `ok` at the top level.
    assert_eq!(json["ok"], true);
    assert_eq!(json["hash"], "abc1234");
    assert!(json.get("action").is_none());
}

#[test]
fn a_commit_that_created_nothing_omits_the_hash() {
    let json = serde_json::to_value(GitCommitResponse {
        action: GitActionResponse::blocked(GitFailure::Failed, "nothing to commit"),
        hash: None,
    })
    .expect("serialises");

    assert_eq!(json["ok"], false);
    assert!(json.get("hash").is_none());
}

#[test]
fn checkout_defaults_leave_the_working_tree_alone() {
    let request: GitCheckoutRequest =
        serde_json::from_str(r#"{"branch":"main"}"#).expect("deserialises");
    assert_eq!(request.branch, "main");
    assert_eq!(request.repo, "");
    assert!(
        !request.carry_changes,
        "carrying changes must be opt-in, never the default"
    );
}

#[test]
fn branch_info_omits_empty_optionals() {
    let json = serde_json::to_value(GitBranchInfo {
        name: "main".into(),
        current: true,
        remote: false,
        upstream: None,
        ahead: 0,
        behind: 0,
        subject: "Root".into(),
        date: "2026-08-14T10:00:00+00:00".into(),
        worktree: None,
    })
    .expect("serialises");

    assert!(json.get("upstream").is_none());
    assert!(json.get("worktree").is_none());
    assert_eq!(json["ahead"], 0);
}

#[test]
fn push_force_is_opt_in() {
    let request: GitPushRequest = serde_json::from_str("{}").expect("deserialises");
    assert!(!request.force);
    assert!(!request.set_upstream);
    assert_eq!(request.remote, None);
}
