use super::*;
use crate::mcp::new_nvim_socket_registry;
use tempfile::tempdir;

#[tokio::test]
async fn session_attaches_and_shutdown_removes_registry_entry() {
    let Some(project) = tempdir().ok() else {
        return;
    };
    let registry = new_nvim_socket_registry();
    let key = SessionKey::new(0, "session-test");
    let session = match NvimSession::start(
        registry.clone(),
        key.clone(),
        project.path(),
        UiSize::default(),
    )
    .await
    {
        Ok(session) => session,
        Err(error) => {
            eprintln!("skipping Neovim process test: {error}");
            return;
        }
    };
    assert!(session.is_alive());
    assert_eq!(
        registry.read().await.get(&(0, "session-test".into())),
        Some(&session.socket_path().to_path_buf())
    );
    session.shutdown().await;
    assert!(!session.is_alive());
    assert!(!registry
        .read()
        .await
        .contains_key(&(0, "session-test".into())));
}
