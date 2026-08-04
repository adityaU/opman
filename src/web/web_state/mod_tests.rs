use super::*;
use crate::web::types::EditorEvent;
use std::path::PathBuf;

#[test]
fn uuid_like_id_is_hex_and_unique() {
    let a = uuid_like_id();
    let b = uuid_like_id();
    assert!(!a.is_empty());
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, b, "two ids should almost never collide");
}

#[tokio::test]
async fn new_test_with_projects_loads_projects() {
    let h = WebStateHandle::new_test_with_projects(vec![
        ("a".into(), PathBuf::from("/a")),
        ("b".into(), PathBuf::from("/b")),
    ]);
    assert_eq!(
        h.all_project_paths().await,
        vec!["/a".to_string(), "/b".to_string()]
    );
    // build_inner defaults: active project 0, all panels visible, TerminalPane focused.
    let state = h.get_state().await;
    assert_eq!(state.active_project, 0);
    assert_eq!(state.focused, "TerminalPane");
    assert!(state.panels.sidebar);
    assert!(state.panels.terminal_pane);
    assert!(state.panels.neovim_pane);
    assert!(state.panels.integrated_terminal);
    assert!(state.panels.git_panel);
}

#[tokio::test]
async fn subscribe_events_receives_broadcast() {
    let h = WebStateHandle::new_test();
    let mut rx = h.subscribe_events();
    h.broadcast_toast("x".into(), "info");
    assert!(rx.try_recv().is_ok());
}

#[tokio::test]
async fn emit_editor_file_changed_without_channel_is_noop() {
    // new_test leaves editor_tx = None → nothing to send, must not panic.
    let h = WebStateHandle::new_test();
    h.emit_editor_file_changed("a.rs", "web_save");
}

#[tokio::test]
async fn emit_editor_file_changed_sends_when_attached() {
    let mut h = WebStateHandle::new_test();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<EditorEvent>(16);
    h.set_editor_tx(tx);
    h.emit_editor_file_changed("src/a.rs", "ai_edit");
    match rx.try_recv() {
        Ok(EditorEvent::FileChanged { path, source }) => {
            assert_eq!(path, "src/a.rs");
            assert_eq!(source, "ai_edit");
        }
        other => panic!("expected FileChanged, got {other:?}"),
    }
}

#[tokio::test]
async fn db_for_test_exposes_working_db() {
    let h = WebStateHandle::new_test();
    // A fresh in-memory DB has no persisted rows.
    assert!(h.db_for_test().list_missions().is_empty());
}
