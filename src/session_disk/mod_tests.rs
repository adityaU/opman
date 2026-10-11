//! Store selection and the error-tolerant `list` front door.

use super::*;

#[test]
fn backend_picks_the_store_its_engine_persists_to() {
    use crate::cli::AgentBackend;
    assert!(matches!(
        SessionStore::for_backend(AgentBackend::Opencode, None),
        Some(SessionStore::Opencode(path)) if path.ends_with("opencode")
    ));
    assert!(matches!(
        SessionStore::for_backend(AgentBackend::ClaudeCode, None),
        Some(SessionStore::ClaudeEngine(path)) if path.ends_with("opman/claude_sessions.json")
    ));
    assert!(matches!(
        SessionStore::for_backend(AgentBackend::ClaudeAcp, Some("claude")),
        Some(SessionStore::Acp(path)) if path.ends_with("opman/acp_claude_sessions.json")
    ));
    assert_eq!(
        SessionStore::for_backend(AgentBackend::ClaudeAcp, None),
        None
    );
}

#[test]
fn an_unreadable_store_lists_nothing_instead_of_failing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("acp_x_sessions.json");
    std::fs::write(&path, "not json").expect("write");
    assert!(SessionStore::Acp(path).list("/proj").is_empty());
}

#[tokio::test]
async fn opencode_falls_back_to_the_json_layout_without_a_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let session_dir = dir.path().join("storage/session/global");
    std::fs::create_dir_all(&session_dir).expect("mkdir");
    std::fs::write(
        session_dir.join("ses_1.json"),
        r#"{"id":"ses_1","directory":"/scratch","title":"Hi","time":{"created":1,"updated":2}}"#,
    )
    .expect("write");
    let store = SessionStore::Opencode(dir.path().to_path_buf());
    let sessions = store.list_async("/scratch").await;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Hi");
}
