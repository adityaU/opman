//! Behaviour of the shared git runner against real repositories.

use super::*;

/// A repository with one commit, built through the runner itself.
async fn repo() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "t@example.com"][..],
        &["config", "user.name", "Test"][..],
        &["commit", "-q", "--allow-empty", "-m", "root"][..],
    ] {
        run_strict(path, args, Reach::Local).await.expect("setup");
    }
    dir
}

#[tokio::test]
async fn success_captures_stdout() {
    let dir = repo().await;
    let out = run_strict(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"], Reach::Local)
        .await
        .expect("ran");
    assert_eq!(out.trimmed(), "main");
}

#[tokio::test]
async fn refusal_is_classified_not_errored() {
    let dir = repo().await;
    let outcome = run(dir.path(), &["checkout", "no-such-branch"], Reach::Local)
        .await
        .expect("spawned");
    let refusal = outcome.expect_err("checkout of a missing branch should refuse");
    assert_eq!(refusal.failure, GitFailure::NotFound);
    assert!(!refusal.detail.is_empty(), "git's reason should survive");
}

#[tokio::test]
async fn dirty_tree_is_classified() {
    let dir = repo().await;
    let path = dir.path();
    std::fs::write(path.join("f.txt"), "one").expect("write");
    run_strict(path, &["add", "f.txt"], Reach::Local).await.expect("add");
    run_strict(path, &["commit", "-q", "-m", "add f"], Reach::Local).await.expect("commit");
    run_strict(path, &["checkout", "-q", "-b", "other"], Reach::Local).await.expect("branch");
    std::fs::write(path.join("f.txt"), "two").expect("write");
    run_strict(path, &["commit", "-qam", "change f"], Reach::Local).await.expect("commit");
    run_strict(path, &["checkout", "-q", "main"], Reach::Local).await.expect("back");
    std::fs::write(path.join("f.txt"), "conflicting").expect("write");

    let refusal = run(path, &["checkout", "other"], Reach::Local)
        .await
        .expect("spawned")
        .expect_err("should refuse");
    assert_eq!(refusal.failure, GitFailure::DirtyTree);
}

#[tokio::test]
async fn lenient_flattens_refusal_to_empty() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let out = run_lenient(dir.path(), &["branch", "--format=%(refname:short)"])
        .await
        .expect("spawned");
    assert_eq!(out.trimmed(), "");
}

#[tokio::test]
async fn network_commands_never_prompt() {
    let dir = repo().await;
    // A remote that cannot be reached must fail fast rather than block on a
    // credential prompt; BatchMode + GIT_TERMINAL_PROMPT are what guarantee it.
    run_strict(
        dir.path(),
        &["remote", "add", "origin", "https://127.0.0.1:1/nope.git"],
        Reach::Local,
    )
    .await
    .expect("add remote");

    let outcome = run(dir.path(), &["fetch", "origin"], Reach::Network)
        .await
        .expect("spawned");
    assert!(outcome.is_err(), "unreachable remote should refuse");
}

#[tokio::test]
async fn lines_skips_blanks() {
    let out = GitOutput {
        stdout: "a\n\n  b  \n".to_string(),
        stderr: String::new(),
    };
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["a", "b"]);
}

#[tokio::test]
async fn summary_prefers_stderr() {
    let out = GitOutput {
        stdout: "out".to_string(),
        stderr: " Switched to branch 'x' ".to_string(),
    };
    assert_eq!(out.summary(), "Switched to branch 'x'");

    let quiet = GitOutput {
        stdout: " out ".to_string(),
        stderr: String::new(),
    };
    assert_eq!(quiet.summary(), "out");
}

#[test]
fn every_failure_carries_a_recovery() {
    for failure in [
        GitFailure::AuthRequired,
        GitFailure::DirtyTree,
        GitFailure::Conflict,
        GitFailure::NotFound,
        GitFailure::Rejected,
        GitFailure::Locked,
        GitFailure::Failed,
    ] {
        assert!(!failure.hint().is_empty());
    }
}

#[test]
fn auth_classification_covers_the_headless_signatures() {
    for text in [
        "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
        "git@github.com: Permission denied (publickey).",
        "fatal: Authentication failed for 'https://example.com/'",
        "Host key verification failed.",
    ] {
        let refusal = GitRefusal {
            failure: GitFailure::classify(text),
            detail: text.into(),
        };
        assert_eq!(refusal.failure, GitFailure::AuthRequired, "{text}");
    }
}
