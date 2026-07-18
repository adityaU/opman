//! Extra branch coverage for `common.rs::resolve_readable_path` — the
//! `~/.claude` allow-list path and the base-canonicalize error branch.

use super::*;

use std::sync::Mutex;

/// Serializes HOME mutation (process-global env).
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as HOME_LOCK;

struct HomeRedirect {
    prev: Option<std::ffi::OsString>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl HomeRedirect {
    fn new(dir: &std::path::Path) -> Self {
        let guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("HOME");
        std::env::set_var("HOME", dir);
        HomeRedirect {
            prev,
            _guard: guard,
        }
    }
}

impl Drop for HomeRedirect {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[test]
fn resolve_readable_path_allows_claude_home() {
    let home = tempfile::TempDir::new().unwrap();
    let claude = home.path().join(".claude");
    std::fs::create_dir_all(&claude).unwrap();
    let target = claude.join("plan.md");
    std::fs::write(&target, "notes").unwrap();

    // A project base that is a *different* real dir (so the target is NOT under it).
    let base = tempfile::TempDir::new().unwrap();

    let _home = HomeRedirect::new(home.path());
    let out = resolve_readable_path(base.path(), &target.to_string_lossy())
        .expect("file under ~/.claude must be allowed");
    // Canonicalized target ends with the file name.
    assert!(out.ends_with("plan.md"));
}

#[test]
fn resolve_readable_path_outside_claude_home_rejected() {
    // HOME points at a temp dir with a .claude, but the target file lives
    // elsewhere → neither project nor ~/.claude → BadRequest.
    let home = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();

    let base = tempfile::TempDir::new().unwrap();
    let other = tempfile::TempDir::new().unwrap();
    let target = other.path().join("secret.txt");
    std::fs::write(&target, "x").unwrap();

    let _home = HomeRedirect::new(home.path());
    let res = resolve_readable_path(base.path(), &target.to_string_lossy());
    assert!(matches!(res, Err(WebError::BadRequest(_))));
}

#[test]
fn resolve_readable_path_base_canonicalize_error() {
    // The target file exists, but the base dir does not → base.canonicalize()
    // fails → Internal error branch. (Target must exist so we reach the base
    // canonicalize; use an absolute path to a real file.)
    let real = tempfile::TempDir::new().unwrap();
    let target = real.path().join("f.txt");
    std::fs::write(&target, "y").unwrap();

    let missing_base = real.path().join("does-not-exist");
    let res = resolve_readable_path(&missing_base, &target.to_string_lossy());
    assert!(matches!(res, Err(WebError::Internal(_))));
}
