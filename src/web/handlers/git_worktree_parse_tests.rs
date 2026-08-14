//! Porcelain parsing and path containment, driven by fixtures rather than a repository.

use super::git_worktree_tests::*;
use super::super::git_worktree::*;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn parses_two_records_with_branches() {
    let text = "worktree /repo\nHEAD abc123\nbranch refs/heads/main\n\n\
                worktree /repo/wt\nHEAD def456\nbranch refs/heads/feature/x\n\n";
    let records = parse_porcelain(text);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].path.as_deref(), Some("/repo"));
    assert_eq!(records[0].head.as_deref(), Some("abc123"));
    assert_eq!(records[0].branch.as_deref(), Some("refs/heads/main"));
    assert_eq!(records[1].branch.as_deref(), Some("refs/heads/feature/x"));
}

#[test]
fn parses_final_record_without_trailing_blank_line() {
    let records = parse_porcelain("worktree /repo\nHEAD abc\n");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].head.as_deref(), Some("abc"));
}

#[test]
fn parses_detached_locked_prunable_and_bare() {
    let text = "worktree /repo\nbare\n\n\
                worktree /repo/d\nHEAD aaa\ndetached\n\n\
                worktree /repo/l\nHEAD bbb\nbranch refs/heads/l\nlocked\n\n\
                worktree /repo/p\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n";
    let records = parse_porcelain(text);
    assert_eq!(records.len(), 4);
    assert!(records[0].bare);
    assert!(records[0].head.is_none());
    assert!(records[1].detached);
    assert!(records[1].branch.is_none());
    assert!(records[2].locked);
    assert_eq!(
        records[3].prunable.as_deref(),
        Some("gitdir file points to non-existent location")
    );
}

#[test]
fn locked_with_reason_still_sets_the_flag() {
    let records = parse_porcelain("worktree /repo/l\nHEAD b\nlocked on a removable drive\n");
    assert!(records[0].locked);
}

#[test]
fn unknown_keys_and_blank_runs_are_ignored() {
    let records = parse_porcelain("\n\nworktree /repo\nsomething else\n\n\n");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].path.as_deref(), Some("/repo"));
}

#[test]
fn empty_output_yields_no_records() {
    assert!(parse_porcelain("").is_empty());
    assert!(parse_porcelain("\n\n").is_empty());
}

#[test]
fn entry_strips_refs_heads_and_computes_relative() {
    let root = TempDir::new().expect("tempdir");
    let wt = root.path().join("wt");
    std::fs::create_dir(&wt).expect("mkdir");
    let mut record = Record::default();
    record.absorb(&format!("worktree {}", wt.to_string_lossy()));
    record.absorb("HEAD abc");
    record.absorb("branch refs/heads/feature/x");

    let entry = to_entry(&record, false, &canon(root.path()), &canon(&wt)).expect("entry");
    assert_eq!(entry.branch.as_deref(), Some("feature/x"));
    assert_eq!(entry.relative.as_deref(), Some("wt"));
    assert!(entry.current);
    assert!(!entry.main);
}

#[test]
fn entry_outside_the_project_has_no_relative() {
    let root = TempDir::new().expect("tempdir");
    let other = TempDir::new().expect("tempdir");
    let mut record = Record::default();
    record.absorb(&format!("worktree {}", other.path().to_string_lossy()));
    let entry = to_entry(&record, true, &canon(root.path()), &canon(root.path())).expect("entry");
    assert!(entry.relative.is_none());
    assert!(!entry.current);
    assert!(entry.head.is_empty());
}

#[test]
fn entry_requires_a_path() {
    let record = Record::default();
    assert!(to_entry(&record, true, Path::new("/a"), Path::new("/a")).is_none());
}

#[test]
fn project_root_itself_is_relative_dot() {
    let root = TempDir::new().expect("tempdir");
    let mut record = Record::default();
    record.absorb(&format!("worktree {}", root.path().to_string_lossy()));
    let entry = to_entry(&record, true, &canon(root.path()), &canon(root.path())).expect("entry");
    assert_eq!(entry.relative.as_deref(), Some("."));
    assert!(entry.current);
}

#[test]
fn resolve_inside_accepts_a_child() {
    let root = Path::new("/project");
    let out = resolve_inside(root, "trees/one").expect("inside");
    assert_eq!(out, Path::new("/project/trees/one"));
}

#[test]
fn resolve_inside_rejects_parent_traversal() {
    let root = Path::new("/project");
    assert!(resolve_inside(root, "../escape").is_err());
    assert!(resolve_inside(root, "a/../../escape").is_err());
}

#[test]
fn resolve_inside_rejects_an_absolute_path_elsewhere() {
    assert!(resolve_inside(Path::new("/project"), "/tmp/elsewhere").is_err());
}

#[test]
fn resolve_inside_normalises_cur_dir() {
    let out = resolve_inside(Path::new("/project"), "./a/./b").expect("inside");
    assert_eq!(out, Path::new("/project/a/b"));
}
