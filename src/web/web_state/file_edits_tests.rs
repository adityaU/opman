use super::*;
use crate::web::web_state::WebStateHandle;
use std::path::Path;

async fn edits_for(h: &WebStateHandle, sid: &str) -> Vec<super::super::FileEditRecord> {
    h.inner
        .read()
        .await
        .file_edits
        .get(sid)
        .cloned()
        .unwrap_or_default()
}

async fn snapshot_for(h: &WebStateHandle, sid: &str, path: &str) -> Option<String> {
    h.inner
        .read()
        .await
        .file_snapshots
        .get(sid)
        .and_then(|m| m.get(path).cloned())
}

fn git_commit(dir: &Path, file: &str, contents: &str) {
    let repo = git2::Repository::init(dir).unwrap();
    std::fs::write(dir.join(file), contents).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(file)).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
}

#[tokio::test]
async fn record_missing_file_is_noop() {
    let h = WebStateHandle::new_test();
    h.record_file_edit("sess", "/nonexistent/path/does-not-exist.txt", None).await;
    assert!(edits_for(&h, "sess").await.is_empty());
}

#[tokio::test]
async fn record_absolute_path_no_git_uses_current_as_original() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("f.txt");
    std::fs::write(&file, "hello").unwrap();

    let abs = file.to_string_lossy().to_string();
    h.record_file_edit("sess", &abs, None).await;

    let edits = edits_for(&h, "sess").await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].path, abs);
    assert_eq!(edits[0].new_content, "hello");
    // No git → original falls back to current content.
    assert_eq!(edits[0].original_content, "hello");
    assert_eq!(edits[0].index, 0);
    // Snapshot stored under the (absolute) file_path key.
    assert_eq!(snapshot_for(&h, "sess", &abs).await.as_deref(), Some("hello"));
}

#[tokio::test]
async fn record_relative_path_with_project_dir_non_git() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("rel.txt"), "content").unwrap();

    h.record_file_edit("sess", "rel.txt", Some(dir.path())).await;

    let edits = edits_for(&h, "sess").await;
    assert_eq!(edits.len(), 1);
    // git show fails (not a repo) → original == new_content.
    assert_eq!(edits[0].original_content, "content");
    assert_eq!(edits[0].new_content, "content");
}

#[tokio::test]
async fn record_relative_path_without_project_dir() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("plain.txt");
    std::fs::write(&file, "body").unwrap();
    // Absolute path but no project dir; get_git_original returns None (dir?).
    h.record_file_edit("sess", &file.to_string_lossy(), None).await;
    let edits = edits_for(&h, "sess").await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].original_content, "body");
}

#[tokio::test]
async fn record_abs_path_project_dir_strip_prefix_fails() {
    let h = WebStateHandle::new_test();
    let file_dir = tempfile::TempDir::new().unwrap();
    let other_dir = tempfile::TempDir::new().unwrap();
    let file = file_dir.path().join("x.txt");
    std::fs::write(&file, "data").unwrap();
    // project_dir does not contain the absolute file → strip_prefix fails → None.
    h.record_file_edit("sess", &file.to_string_lossy(), Some(other_dir.path())).await;
    let edits = edits_for(&h, "sess").await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].original_content, "data");
}

#[tokio::test]
async fn record_with_git_original_from_head() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    git_commit(dir.path(), "tracked.txt", "committed version\n");
    // Modify the working copy after the commit.
    std::fs::write(dir.path().join("tracked.txt"), "working version\n").unwrap();

    h.record_file_edit("sess", "tracked.txt", Some(dir.path())).await;

    let edits = edits_for(&h, "sess").await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_content, "working version\n");
    // Original comes from git HEAD.
    assert_eq!(edits[0].original_content, "committed version\n");
}

#[tokio::test]
async fn record_second_edit_reuses_snapshot() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("f.txt");
    std::fs::write(&file, "v1").unwrap();
    let abs = file.to_string_lossy().to_string();

    h.record_file_edit("sess", &abs, None).await;
    // Change content and record again — snapshot from the first edit is reused.
    std::fs::write(&file, "v2").unwrap();
    h.record_file_edit("sess", &abs, None).await;

    let edits = edits_for(&h, "sess").await;
    assert_eq!(edits.len(), 2);
    // Both edits carry the original snapshot ("v1").
    assert_eq!(edits[0].original_content, "v1");
    assert_eq!(edits[1].original_content, "v1");
    assert_eq!(edits[1].new_content, "v2");
    assert_eq!(edits[1].index, 1);
}

#[tokio::test]
async fn record_caps_edits_per_session() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("f.txt");
    std::fs::write(&file, "x").unwrap();
    let abs = file.to_string_lossy().to_string();

    for _ in 0..205 {
        h.record_file_edit("sess", &abs, None).await;
    }
    let edits = edits_for(&h, "sess").await;
    // Capped at MAX_EDITS_PER_SESSION (200).
    assert_eq!(edits.len(), 200);
}

#[tokio::test]
async fn clear_file_edits_removes_all() {
    let h = WebStateHandle::new_test();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("f.txt");
    std::fs::write(&file, "x").unwrap();
    let abs = file.to_string_lossy().to_string();
    h.record_file_edit("sess", &abs, None).await;
    assert_eq!(edits_for(&h, "sess").await.len(), 1);

    h.clear_file_edits("sess").await;
    assert!(edits_for(&h, "sess").await.is_empty());
    assert!(snapshot_for(&h, "sess", &abs).await.is_none());
}
