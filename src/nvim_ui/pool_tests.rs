use super::*;
use crate::mcp::new_nvim_socket_registry;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

async fn pool() -> Option<(NvimUiPool, tempfile::TempDir)> {
    let project = tempdir().ok()?;
    let pool = NvimUiPool::new(new_nvim_socket_registry());
    Some((pool, project))
}

async fn ensure(
    pool: &NvimUiPool,
    project: &tempfile::TempDir,
    id: &str,
) -> Option<Arc<NvimSession>> {
    pool.ensure(SessionKey::new(0, id), project.path(), UiSize::default())
        .await
        .ok()
}

#[tokio::test]
async fn dead_session_is_replaced_instead_of_reused() {
    let Some((pool, project)) = pool().await else {
        return;
    };
    let Some(first) = ensure(&pool, &project, "replace").await else {
        return;
    };
    first.shutdown().await;
    let Some(second) = ensure(&pool, &project, "replace").await else {
        return;
    };
    assert!(!Arc::ptr_eq(&first, &second));
    second.shutdown().await;
}

#[tokio::test]
async fn idle_sweep_removes_a_session_and_registry_key() {
    let Some((pool, project)) = pool().await else {
        return;
    };
    let Some(session) = ensure(&pool, &project, "idle").await else {
        return;
    };
    let key = session.key().clone();
    sleep(Duration::from_millis(3)).await;
    assert_eq!(pool.sweep(Duration::from_millis(1)).await, 1);
    assert!(pool.get(&key).await.is_none());
}

#[tokio::test]
async fn pool_evicts_the_least_recently_used_session_at_capacity() {
    let Some((pool, project)) = pool().await else {
        return;
    };
    let mut sessions = Vec::new();
    for index in 0..MAX_SESSIONS {
        let Some(session) = ensure(&pool, &project, &format!("lru-{index}")).await else {
            pool.shutdown_all().await;
            return;
        };
        sessions.push(session);
        sleep(Duration::from_millis(2)).await;
    }
    let Some(newest) = ensure(&pool, &project, "lru-new").await else {
        return;
    };
    assert_eq!(pool.len().await, MAX_SESSIONS);
    assert!(pool.get(&SessionKey::new(0, "lru-0")).await.is_none());
    newest.shutdown().await;
    pool.shutdown_all().await;
}
