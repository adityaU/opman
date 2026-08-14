//! Validation boundaries for the git argv newtypes.

use super::*;

#[test]
fn accepts_ordinary_branch_names() {
    for name in ["main", "feature/login", "release-1.2", "origin/main"] {
        assert!(RefName::parse(name).is_ok(), "{name} should parse");
    }
}

#[test]
fn rejects_argv_injection_and_revision_syntax() {
    for name in [
        "",
        "--upload-pack=evil",
        "-b",
        "a..b",
        "HEAD~1",
        "HEAD^",
        "a:b",
        "star*",
        "glob?",
        "brack[et",
        "back\\slash",
        "two\nlines",
        "trailing/",
        "branch.lock",
    ] {
        assert!(RefName::parse(name).is_err(), "{name:?} should be rejected");
    }
}

#[test]
fn split_remote_matches_known_remotes_only() {
    let remotes = vec!["origin".to_string(), "upstream".to_string()];

    let tracked = RefName::parse("origin/feature/login").expect("valid");
    assert_eq!(tracked.split_remote(&remotes), Some(("origin", "feature/login")));

    // A local branch that merely looks remote-shaped stays local.
    let local = RefName::parse("feature/login").expect("valid");
    assert_eq!(local.split_remote(&remotes), None);

    // The remote name alone is not a branch.
    let bare = RefName::parse("origin").expect("valid");
    assert_eq!(bare.split_remote(&remotes), None);
}

#[test]
fn commit_hash_is_hex_and_bounded() {
    assert!(CommitHash::parse("a1b2c3d").is_ok());
    assert!(CommitHash::parse("").is_err());
    assert!(CommitHash::parse("nothex").is_err());
    assert!(CommitHash::parse(&"a".repeat(65)).is_err());
}

#[test]
fn stash_ref_requires_exact_shape() {
    assert!(StashRef::parse("stash@{0}").is_ok());
    assert!(StashRef::parse("stash@{12}").is_ok());
    for bad in ["stash@{}", "stash@{a}", "-stash@{0}", "stash@{0", "0"] {
        assert!(StashRef::parse(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn repo_path_rejects_option_lookalikes() {
    assert!(RepoPath::parse("src/main.rs").is_ok());
    assert!(RepoPath::parse("--force").is_err());
    assert!(RepoPath::parse("").is_err());
}

#[test]
fn message_trims_and_rejects_empty() {
    assert_eq!(message("  fix: thing  ").expect("valid"), "fix: thing");
    assert!(message("   ").is_err());
    assert!(message("has\0nul").is_err());
}
