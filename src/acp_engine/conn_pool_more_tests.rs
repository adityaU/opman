//! ACP pooling under eviction, concurrency, and against a live agent.

use super::conn_pool_tests::*;

#[tokio::test]
async fn eviction_unbinds_every_session_and_next_use_loads_on_a_new_process() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir]);
    for session in &sessions {
        h.engine.conns.ensure(&h.engine, &session.id).await.unwrap();
    }
    let affected = h.engine.conns.evict(&sessions[0].id).await;
    assert_eq!(affected.len(), 1);
    assert!(h.engine.conns.existing(&sessions[0].id).await.is_none());
    assert!(h.engine.conns.existing(&sessions[1].id).await.is_none());
    h.engine
        .conns
        .ensure(&h.engine, &sessions[0].id)
        .await
        .unwrap();
    assert_eq!(h.initialize_count(), 2);
    assert_eq!(h.event_count("load"), 1);
    h.engine.conns.close_all().await;
}

#[tokio::test]
async fn concurrent_first_use_spawns_and_initializes_once() {
    let h = Harness::new(true);
    let dir = h.dir("project");
    let sessions = h.sessions(&[dir.clone(), dir]);
    let (first, second) = tokio::join!(
        h.engine.conns.ensure(&h.engine, &sessions[0].id),
        h.engine.conns.ensure(&h.engine, &sessions[1].id),
    );
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(h.initialize_count(), 1);
    h.engine.conns.close_all().await;
}
