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
