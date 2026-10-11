//! opencode's pre-SQLite JSON layout, built in a temp dir.

use super::*;

use serde_json::json;

fn write(path: &Path, value: serde_json::Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, value.to_string()).expect("write");
}

fn session(storage: &Path, project: &str, id: &str, directory: &str, updated: u64) {
    write(
        &storage
            .join("session")
            .join(project)
            .join(format!("{id}.json")),
        json!({
            "id": id, "projectID": project, "directory": directory, "title": format!("T {id}"),
            "version": "0.5", "time": { "created": updated - 1, "updated": updated },
        }),
    );
}

#[test]
fn lists_sessions_of_the_project_whose_worktree_holds_the_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = dir.path();
    write(
        &storage.join("project/abc.json"),
        json!({ "id": "abc", "worktree": "/repo" }),
    );
    write(
        &storage.join("project/zzz.json"),
        json!({ "id": "zzz", "worktree": "/other" }),
    );
    session(storage, "abc", "ses_1", "/repo", 10);
    session(storage, "abc", "ses_2", "/repo", 20);
    session(storage, "abc", "ses_sub", "/repo/sub", 30);
    session(storage, "zzz", "ses_x", "/repo", 40);
    write(
        &storage.join("session/abc/ses_child.json"),
        json!({ "id": "ses_child", "directory": "/repo", "parentID": "ses_1",
                "time": { "created": 1, "updated": 15 } }),
    );

    let sessions = list(storage, "/repo", 100).expect("list");
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["ses_2", "ses_child", "ses_1"]);
    assert_eq!(sessions[0].title, "T ses_2");
    assert_eq!(sessions[0].time.updated, 20);
    assert_eq!(sessions[1].parent_id, "ses_1");
}

#[test]
fn directories_outside_a_repository_come_from_the_global_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    session(dir.path(), "global", "ses_g", "/scratch", 5);
    let sessions = list(dir.path(), "/scratch", 100).expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "ses_g");
}

#[test]
fn the_limit_bounds_the_result_and_junk_files_are_skipped() {
    let dir = tempfile::tempdir().expect("tempdir");
    for n in 1..=5 {
        session(
            dir.path(),
            "global",
            &format!("ses_{n}"),
            "/scratch",
            n * 10,
        );
    }
    std::fs::write(dir.path().join("session/global/broken.json"), "{").expect("write");
    std::fs::write(dir.path().join("session/global/notes.txt"), "x").expect("write");
    let sessions = list(dir.path(), "/scratch", 2).expect("list");
    assert_eq!(sessions.len(), 2);
    assert!(sessions[0].time.updated >= sessions[1].time.updated);
}

#[test]
fn a_missing_storage_directory_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(list(&dir.path().join("absent"), "/repo", 100)
        .expect("list")
        .is_empty());
}
