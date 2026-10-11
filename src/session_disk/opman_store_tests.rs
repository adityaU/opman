//! The registries opman's own engines persist, in the shapes those engines write.

use super::*;

use serde_json::json;

fn file(dir: &tempfile::TempDir, name: &str, value: serde_json::Value) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, value.to_string()).expect("write");
    path
}

#[test]
fn acp_registry_lists_the_directory_newest_created_first_with_choices() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = file(
        &dir,
        "acp_claude_sessions.json",
        json!([
            { "id": "ses_a", "title": "First", "directory": "/proj", "parent_id": "",
              "created": 1, "updated": 50, "acp_session": "uuid-a", "model": "opus",
              "agent": null, "effort": "high", "permission_mode": "default",
              "title_locked": false },
            { "id": "ses_b", "title": "Second", "directory": "/proj", "parent_id": "",
              "created": 2, "updated": 3 },
            { "id": "ses_c", "title": "Elsewhere", "directory": "/other", "parent_id": "",
              "created": 9, "updated": 9 },
        ]),
    );
    let sessions = list_acp_registry(&path, "/proj").expect("list");
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["ses_b", "ses_a"]);
    let first = &sessions[1];
    assert_eq!(first.title, "First");
    assert_eq!(first.time.updated, 50);
    let engine = serde_json::to_value(&first.engine).expect("json");
    assert_eq!(engine["model"], "opus");
    assert_eq!(engine["effort"], "high");
    assert!(engine["agent"].is_null());
}

#[test]
fn claude_registry_reads_the_keyed_map_and_keeps_subagent_rows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = file(
        &dir,
        "claude_sessions.json",
        json!({
            "sessions": {
                "ses_p": { "id": "ses_p", "title": "Parent", "directory": "/proj",
                           "parent_id": "", "created": 5, "updated": 6,
                           "claude_session_id": "uuid", "lineage": ["uuid"] },
                "agent1": { "id": "agent1", "title": "Sub", "directory": "/proj",
                            "parent_id": "ses_p", "created": 7, "updated": 7,
                            "is_subagent": true },
            },
            "deleted": ["gone"],
        }),
    );
    let sessions = list_claude_registry(&path, "/proj").expect("list");
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["agent1", "ses_p"]);
    assert_eq!(sessions[0].parent_id, "ses_p");
}

#[test]
fn a_registry_never_written_is_empty_and_a_corrupt_one_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let absent = dir.path().join("acp_codex_sessions.json");
    assert!(list_acp_registry(&absent, "/proj")
        .expect("list")
        .is_empty());
    assert!(list_claude_registry(&absent, "/proj")
        .expect("list")
        .is_empty());
    let corrupt = file(&dir, "bad.json", json!({ "not": "a list" }));
    assert!(list_acp_registry(&corrupt, "/proj").is_err());
}
