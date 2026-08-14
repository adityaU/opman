//! Pull, stash and gitignore type shapes.

use super::*;
use serde_json::json;

#[test]
fn git_pull_request_and_response() {
    let r: GitPullRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(r.remote, "");
    assert_eq!(r.branch, "");
    assert_eq!(r.repo, "");
    let r2: GitPullRequest =
        serde_json::from_value(json!({"remote": "origin", "branch": "main", "repo": "r"})).unwrap();
    assert_eq!(r2.remote, "origin");

    let resp = GitPullResponse {
        success: true,
        output: "up to date".into(),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["success"], true);
    assert_eq!(v["output"], "up to date");
}

#[test]
fn git_ignore_request_default_action() {
    let r: GitIgnoreRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(r.action, "list");
    assert!(r.patterns.is_empty());
    assert_eq!(r.repo, "");
    let r2: GitIgnoreRequest = serde_json::from_value(json!({
        "action": "add", "patterns": ["*.log"], "repo": "r"
    }))
    .unwrap();
    assert_eq!(r2.action, "add");
    assert_eq!(r2.patterns, vec!["*.log".to_string()]);
}

#[test]
fn gitignore_action_default_helper() {
    assert_eq!(gitignore_action_default(), "list");
}

#[test]
fn git_ignore_response() {
    let resp = GitIgnoreResponse {
        success: true,
        content: "*.log\n".into(),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["success"], true);
    assert_eq!(v["content"], "*.log\n");
}
