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
async fn push_activity_event_and_feed() {
    let h = WebStateHandle::new_test();
    assert!(h.get_activity_feed("s1").await.is_empty());
    let ev = ActivityEventPayload {
        session_id: "s1".into(),
        kind: "tool_call".into(),
        summary: "ran a tool".into(),
        detail: Some("bash".into()),
        timestamp: "t0".into(),
    };
    let mut rx = h.subscribe_events();
    h.push_activity_event(ev).await;
    assert!(matches!(rx.try_recv(), Ok(WebEvent::ActivityEvent(_))));
    let feed = h.get_activity_feed("s1").await;
    assert_eq!(feed.len(), 1);
    assert_eq!(feed[0].kind, "tool_call");
}

#[tokio::test]
async fn push_activity_event_ring_buffer_caps_at_200() {
    let h = WebStateHandle::new_test();
    for i in 0..205 {
        h.push_activity_event(ActivityEventPayload {
            session_id: "s1".into(),
            kind: "status".into(),
            summary: format!("e{i}"),
            detail: None,
            timestamp: format!("t{i}"),
        })
        .await;
    }
    let feed = h.get_activity_feed("s1").await;
    assert_eq!(feed.len(), 200);
    // Oldest 5 drained → first retained event is e5.
    assert_eq!(feed[0].summary, "e5");
}

#[tokio::test]
async fn push_activity_event_prunes_excess_sessions() {
    let h = WebStateHandle::new_test();
    for i in 0..51 {
        h.push_activity_event(ActivityEventPayload {
            session_id: format!("s{i}"),
            kind: "status".into(),
            summary: "x".into(),
            detail: None,
            timestamp: "t".into(),
        })
        .await;
    }
    let count = h.inner.read().await.activity_log.len();
    assert!(count <= 50, "expected <=50 sessions, got {count}");
}

#[tokio::test]
async fn clear_activity_log_removes_session() {
    let h = WebStateHandle::new_test();
    h.push_activity_event(ActivityEventPayload {
        session_id: "s1".into(),
        kind: "status".into(),
        summary: "x".into(),
        detail: None,
        timestamp: "t".into(),
    })
    .await;
    assert_eq!(h.get_activity_feed("s1").await.len(), 1);
    h.clear_activity_log("s1").await;
    assert!(h.get_activity_feed("s1").await.is_empty());
}

#[tokio::test]
async fn spawn_presence_cleanup_does_not_panic() {
    // Exercises the spawn call itself. The 60s loop body is not driven here.
    let h = WebStateHandle::new_test();
    h.spawn_presence_cleanup();
}
