use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

fn client(id: &str, last_seen: String) -> ClientPresence {
    ClientPresence {
        client_id: id.into(),
        interface_type: "web".into(),
        focused_session: Some("s1".into()),
        last_seen,
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[tokio::test]
async fn register_get_deregister_presence() {
    let h = WebStateHandle::new_test();
    let mut rx = h.subscribe_events();
    h.register_presence(&client("c1", now_rfc3339())).await;
    assert!(matches!(rx.try_recv(), Ok(WebEvent::PresenceChanged(_))));
    let snap = h.get_presence().await;
    assert_eq!(snap.clients.len(), 1);
    assert_eq!(snap.clients[0].client_id, "c1");

    h.deregister_presence("c1").await;
    assert!(h.get_presence().await.clients.is_empty());
    assert!(matches!(rx.try_recv(), Ok(WebEvent::PresenceChanged(_))));
}

#[tokio::test]
async fn evict_stale_clients_removes_stale_and_unparseable() {
    let h = WebStateHandle::new_test();
    let fresh = client("fresh", now_rfc3339());
    let stale = client(
        "stale",
        (chrono::Utc::now() - chrono::Duration::seconds(200)).to_rfc3339(),
    );
    let bad = client("bad", "not-a-timestamp".into());
    h.register_presence(&fresh).await;
    h.register_presence(&stale).await;
    h.register_presence(&bad).await;

    h.evict_stale_clients().await;
    let snap = h.get_presence().await;
    assert_eq!(snap.clients.len(), 1);
    assert_eq!(snap.clients[0].client_id, "fresh");
}

#[tokio::test]
async fn evict_stale_clients_no_change_when_all_fresh() {
    let h = WebStateHandle::new_test();
    h.register_presence(&client("c1", now_rfc3339())).await;
    let mut rx = h.subscribe_events();
    let _ = rx.try_recv(); // drain register event
    h.evict_stale_clients().await;
    // Nothing changed → no PresenceChanged event emitted.
    assert!(rx.try_recv().is_err());
    assert_eq!(h.get_presence().await.clients.len(), 1);
}

#[tokio::test]
async fn spawn_presence_cleanup_does_not_panic() {
    // Exercises the spawn call itself. The 60s loop body is not driven here.
    let h = WebStateHandle::new_test();
    h.spawn_presence_cleanup();
}
