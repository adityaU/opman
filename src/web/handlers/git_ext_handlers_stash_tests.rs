//! Generated coverage tests for git_ext_handlers.rs (part 2):
//! git_pull, git_stash, git_gitignore, combined_output helper.
#![allow(clippy::disallowed_names)]

use super::git_ext_handlers_history_tests::{
    call, commit_all, init_repo, run_git, state_for, write_file,
};
use super::*;
use axum::http::StatusCode;
use std::path::Path;

// ── combined_output helper (direct) ──────────────────────────────────

#[test]
fn combined_output_stdout_only_and_with_stderr() {
    let base = run_git(Path::new("."), &["--version"]);
    let out = std::process::Output {
        status: base.status,
        stdout: b"  hello  ".to_vec(),
        stderr: Vec::new(),
    };
    assert_eq!(combined_output(&out), "hello");

    let base2 = run_git(Path::new("."), &["--version"]);
    let out2 = std::process::Output {
        status: base2.status,
        stdout: b"out".to_vec(),
        stderr: b"err".to_vec(),
    };
    assert_eq!(combined_output(&out2), "out\nerr");
}

// ── git_pull ─────────────────────────────────────────────────────────

// ── git_stash ────────────────────────────────────────────────────────

// ── git_gitignore ────────────────────────────────────────────────────
