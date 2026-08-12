use super::*;
use crate::mcp::new_nvim_socket_registry;
use std::path::PathBuf;

#[tokio::test]
async fn publish_then_drop_removes_only_our_entry() {
    let registry = new_nvim_socket_registry();
    let key = SessionKey::new(3, "session");
    let path = PathBuf::from("/run/user/1000/nvim-ui-test.sock");
    let guard = RegistryGuard::publish(registry.clone(), &key, &path)
        .await
        .unwrap();
    assert_eq!(
        registry.read().await.get(&(3, "session".into())),
        Some(&path)
    );
    drop(guard);
    tokio::task::yield_now().await;
    assert!(!registry.read().await.contains_key(&(3, "session".into())));
}

#[tokio::test]
async fn an_old_guard_cannot_remove_a_replacement() {
    let registry = new_nvim_socket_registry();
    let key = SessionKey::new(1, "same");
    let first_path = PathBuf::from("first");
    let first = RegistryGuard::publish(registry.clone(), &key, &first_path)
        .await
        .unwrap();
    let second_path = PathBuf::from("second");
    let second = RegistryGuard::publish(registry.clone(), &key, &second_path)
        .await
        .unwrap();
    drop(first);
    tokio::task::yield_now().await;
    assert_eq!(
        registry.read().await.get(&(1, "same".into())),
        Some(&second_path)
    );
    drop(second);
}
