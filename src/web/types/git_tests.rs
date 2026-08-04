use super::*;
use serde_json::json;

#[test]
fn git_file_entry_and_status_response() {
    let e = GitFileEntry {
        path: "a.rs".into(),
        status: "M".into(),
    };
    let v = serde_json::to_value(&e).unwrap();
    assert_eq!(v["path"], "a.rs");
    assert_eq!(v["status"], "M");
    let _ = e.clone();

    let resp = GitStatusResponse {
        branch: "main".into(),
        staged: vec![e.clone()],
        unstaged: vec![],
        untracked: vec![e],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["branch"], "main");
    assert_eq!(v["staged"].as_array().unwrap().len(), 1);
    assert_eq!(v["unstaged"], json!([]));
    assert_eq!(v["untracked"].as_array().unwrap().len(), 1);
}

#[test]
fn git_diff_response_and_query() {
    let resp = GitDiffResponse {
        diff: "@@ -1 +1 @@".into(),
    };
    assert_eq!(serde_json::to_value(&resp).unwrap()["diff"], "@@ -1 +1 @@");

    let q: GitDiffQuery = serde_json::from_value(json!({})).unwrap();
    assert!(q.file.is_none());
    assert!(!q.staged);
    assert_eq!(q.repo, "");

    let q2: GitDiffQuery = serde_json::from_value(json!({
        "file": "a.rs", "staged": true, "repo": "packages/core"
    }))
    .unwrap();
    assert_eq!(q2.file, Some("a.rs".into()));
    assert!(q2.staged);
    assert_eq!(q2.repo, "packages/core");
}

#[test]
fn git_log_entry_and_response_and_query() {
    let c = GitLogEntry {
        hash: "abc123".into(),
        short_hash: "abc".into(),
        author: "me".into(),
        date: "2026".into(),
        message: "init".into(),
    };
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["short_hash"], "abc");

    let resp = GitLogResponse { commits: vec![c] };
    assert_eq!(
        serde_json::to_value(&resp).unwrap()["commits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let q: GitLogQuery = serde_json::from_value(json!({})).unwrap();
    assert!(q.limit.is_none());
    assert_eq!(q.repo, "");
    let q2: GitLogQuery = serde_json::from_value(json!({"limit": 10, "repo": "r"})).unwrap();
    assert_eq!(q2.limit, Some(10));
    assert_eq!(q2.repo, "r");
}

#[test]
fn git_stage_unstage_requests() {
    let s: GitStageRequest = serde_json::from_value(json!({"files": ["a", "b"]})).unwrap();
    assert_eq!(s.files.len(), 2);
    assert_eq!(s.repo, "");
    let s2: GitStageRequest = serde_json::from_value(json!({"files": [], "repo": "r"})).unwrap();
    assert_eq!(s2.repo, "r");

    let u: GitUnstageRequest = serde_json::from_value(json!({"files": ["a"]})).unwrap();
    assert_eq!(u.files, vec!["a".to_string()]);
    assert_eq!(u.repo, "");
}

#[test]
fn git_commit_request_and_response() {
    let r: GitCommitRequest = serde_json::from_value(json!({"message": "msg"})).unwrap();
    assert_eq!(r.message, "msg");
    assert_eq!(r.repo, "");
    let resp = GitCommitResponse {
        hash: "h".into(),
        message: "msg".into(),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["hash"], "h");
    assert_eq!(v["message"], "msg");
}

#[test]
fn git_discard_request() {
    let r: GitDiscardRequest = serde_json::from_value(json!({"files": ["a"]})).unwrap();
    assert_eq!(r.files, vec!["a".to_string()]);
    assert_eq!(r.repo, "");
}

#[test]
fn git_show_query_response_and_file() {
    let q: GitShowQuery = serde_json::from_value(json!({"hash": "abc"})).unwrap();
    assert_eq!(q.hash, "abc");
    assert_eq!(q.repo, "");

    let resp = GitShowResponse {
        hash: "abc".into(),
        author: "me".into(),
        date: "d".into(),
        message: "m".into(),
        diff: "diff".into(),
        files: vec![GitShowFile {
            path: "a.rs".into(),
            status: "M".into(),
        }],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["hash"], "abc");
    assert_eq!(v["files"][0]["status"], "M");
}

#[test]
fn git_branches_response() {
    let resp = GitBranchesResponse {
        current: "main".into(),
        local: vec!["main".into(), "dev".into()],
        remote: vec!["origin/main".into()],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["current"], "main");
    assert_eq!(v["local"].as_array().unwrap().len(), 2);
    assert_eq!(v["remote"][0], "origin/main");
}

#[test]
fn git_checkout_request_and_response_skip() {
    let r: GitCheckoutRequest = serde_json::from_value(json!({"branch": "dev"})).unwrap();
    assert_eq!(r.branch, "dev");
    assert_eq!(r.repo, "");

    let ok = GitCheckoutResponse {
        branch: "dev".into(),
        success: true,
        message: None,
    };
    let v = serde_json::to_value(&ok).unwrap();
    assert!(v.get("message").is_none());
    assert_eq!(v["success"], true);

    let fail = GitCheckoutResponse {
        branch: "dev".into(),
        success: false,
        message: Some("error".into()),
    };
    assert_eq!(serde_json::to_value(&fail).unwrap()["message"], "error");
}

#[test]
fn git_range_diff_query_and_response() {
    let q: GitRangeDiffQuery = serde_json::from_value(json!({})).unwrap();
    assert!(q.base.is_none());
    assert!(q.limit.is_none());
    assert_eq!(q.repo, "");
    let q2: GitRangeDiffQuery =
        serde_json::from_value(json!({"base": "main", "limit": 5, "repo": "r"})).unwrap();
    assert_eq!(q2.base, Some("main".into()));
    assert_eq!(q2.limit, Some(5));

    let resp = GitRangeDiffResponse {
        branch: "dev".into(),
        base: "main".into(),
        commits: vec![],
        diff: "d".into(),
        files_changed: 2,
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["files_changed"], 2);
    assert_eq!(v["base"], "main");
}

#[test]
fn git_context_summary_response() {
    let resp = GitContextSummaryResponse {
        branch: "main".into(),
        recent_commits: vec![],
        staged_count: 1,
        unstaged_count: 2,
        untracked_count: 3,
        summary: "sum".into(),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["staged_count"], 1);
    assert_eq!(v["untracked_count"], 3);
    assert_eq!(v["summary"], "sum");
}

#[test]
fn git_repo_entry_and_repos_response() {
    let e = GitRepoEntry {
        path: ".".into(),
        name: "root".into(),
        branch: "main".into(),
        staged_count: 0,
        unstaged_count: 1,
        untracked_count: 2,
    };
    let _ = e.clone();
    let resp = GitReposResponse { repos: vec![e] };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["repos"][0]["name"], "root");
    assert_eq!(v["repos"][0]["unstaged_count"], 1);
}

#[test]
fn git_repo_scope_default_and_value() {
    let d = GitRepoScope::default();
    assert_eq!(d.repo, "");
    let s: GitRepoScope = serde_json::from_value(json!({})).unwrap();
    assert_eq!(s.repo, "");
    let s2: GitRepoScope = serde_json::from_value(json!({"repo": "sub"})).unwrap();
    assert_eq!(s2.repo, "sub");
}

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
fn git_stash_request_default_action() {
    let r: GitStashRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(r.action, "push");
    assert_eq!(r.message, "");
    assert_eq!(r.stash_ref, "");
    assert_eq!(r.repo, "");
    let r2: GitStashRequest = serde_json::from_value(json!({
        "action": "pop", "message": "m", "stash_ref": "stash@{0}", "repo": "r"
    }))
    .unwrap();
    assert_eq!(r2.action, "pop");
    assert_eq!(r2.stash_ref, "stash@{0}");
}

#[test]
fn stash_action_default_helper() {
    assert_eq!(stash_action_default(), "push");
}

#[test]
fn git_stash_entry_and_response_skip_empty() {
    let empty = GitStashResponse {
        success: true,
        output: "ok".into(),
        entries: vec![],
    };
    let v = serde_json::to_value(&empty).unwrap();
    assert!(v.get("entries").is_none(), "empty entries skipped");

    let full = GitStashResponse {
        success: true,
        output: "listed".into(),
        entries: vec![GitStashEntry {
            index: 0,
            reference: "stash@{0}".into(),
            message: "wip".into(),
        }],
    };
    let v = serde_json::to_value(&full).unwrap();
    assert_eq!(v["entries"][0]["reference"], "stash@{0}");
    assert_eq!(v["entries"][0]["index"], 0);
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
