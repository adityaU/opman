use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

fn perm(id: &str, sid: &str, tool: &str, desc: Option<&str>, time: f64) -> PermissionInput {
    PermissionInput {
        id: id.to_string(),
        session_id: sid.to_string(),
        tool_name: tool.to_string(),
        description: desc.map(|s| s.to_string()),
        time,
    }
}

fn question(id: &str, sid: &str, title: &str, time: f64) -> QuestionInput {
    QuestionInput {
        id: id.to_string(),
        session_id: sid.to_string(),
        title: title.to_string(),
        time,
    }
}

fn signal(id: &str, kind: &str, title: &str, created_at: f64, sid: Option<&str>) -> SignalInput {
    SignalInput {
        id: id.to_string(),
        kind: kind.to_string(),
        title: title.to_string(),
        body: format!("body-{id}"),
        created_at,
        session_id: sid.map(|s| s.to_string()),
    }
}

#[tokio::test]
async fn build_inbox_empty() {
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![],
            questions: vec![],
            watcher_status: None,
            signals: vec![],
        })
        .await;
    assert!(items.is_empty());
}

#[tokio::test]
async fn build_inbox_permission_with_and_without_description() {
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![
                perm("p1", "sess-a", "bash", Some("run a command"), 100.0),
                perm("p2", "sess-b", "edit", None, 200.0),
            ],
            questions: vec![],
            watcher_status: None,
            signals: vec![],
        })
        .await;
    assert_eq!(items.len(), 2);
    // Both high priority; sorted by created_at desc → p2 first.
    assert_eq!(items[0].id, "inbox-perm-p2");
    assert_eq!(items[1].id, "inbox-perm-p1");
    // Description present used verbatim.
    assert_eq!(items[1].description, "run a command");
    // Description absent → synthesized fallback.
    assert_eq!(items[0].description, "sess-b wants to use edit");
    assert!(matches!(items[0].source, InboxItemSource::Permission));
    assert!(matches!(items[0].priority, InboxItemPriority::High));
    assert!(matches!(items[0].state, InboxItemState::Unresolved));
    assert_eq!(items[0].session_id.as_deref(), Some("sess-b"));
}

#[tokio::test]
async fn build_inbox_question() {
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![],
            questions: vec![question("q1", "sess-q", "Pick a branch", 50.0)],
            watcher_status: None,
            signals: vec![],
        })
        .await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "inbox-q-q1");
    assert_eq!(items[0].title, "Question: Pick a branch");
    assert_eq!(items[0].description, "Session sess-q needs your input");
    assert!(matches!(items[0].source, InboxItemSource::Question));
}

#[tokio::test]
async fn build_inbox_watcher_triggered_long_and_short_id() {
    // Long session id (> 8 chars) is truncated for the description.
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![],
            questions: vec![],
            watcher_status: Some(WatcherStatusInput {
                session_id: "abcdefghijkl".to_string(),
                action: "triggered".to_string(),
                idle_since_secs: Some(5),
            }),
            signals: vec![],
        })
        .await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "inbox-watcher-abcdefghijkl");
    assert_eq!(items[0].description, "Session abcdefgh watcher fired");
    assert!(matches!(items[0].priority, InboxItemPriority::Medium));
    assert!(matches!(items[0].source, InboxItemSource::Watcher));

    // Short session id (< 8 chars) uses the whole id.
    let items2 = h
        .build_inbox(InboxRequest {
            permissions: vec![],
            questions: vec![],
            watcher_status: Some(WatcherStatusInput {
                session_id: "s1".to_string(),
                action: "triggered".to_string(),
                idle_since_secs: None,
            }),
            signals: vec![],
        })
        .await;
    assert_eq!(items2[0].description, "Session s1 watcher fired");
}

#[tokio::test]
async fn build_inbox_watcher_not_triggered_is_skipped() {
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![],
            questions: vec![],
            watcher_status: Some(WatcherStatusInput {
                session_id: "sess".to_string(),
                action: "cleared".to_string(),
                idle_since_secs: None,
            }),
            signals: vec![],
        })
        .await;
    assert!(items.is_empty());
}

#[tokio::test]
async fn build_inbox_signals_priority_by_kind() {
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![],
            questions: vec![],
            watcher_status: None,
            signals: vec![
                signal(
                    "s1",
                    "watcher_trigger",
                    "Watcher medium",
                    10.0,
                    Some("sess"),
                ),
                signal("s2", "completion", "Done low", 20.0, None),
            ],
        })
        .await;
    assert_eq!(items.len(), 2);
    // Medium sorts before low regardless of created_at.
    assert_eq!(items[0].id, "inbox-signal-s1");
    assert!(matches!(items[0].priority, InboxItemPriority::Medium));
    assert_eq!(items[0].session_id.as_deref(), Some("sess"));
    assert_eq!(items[1].id, "inbox-signal-s2");
    assert!(matches!(items[1].priority, InboxItemPriority::Low));
    assert!(matches!(items[1].source, InboxItemSource::Completion));
    assert_eq!(items[1].session_id, None);
}

#[tokio::test]
async fn build_inbox_full_priority_sort() {
    let h = WebStateHandle::new_test();
    let items = h
        .build_inbox(InboxRequest {
            permissions: vec![perm("p1", "s", "bash", None, 1.0)],
            questions: vec![question("q1", "s", "Q", 2.0)],
            watcher_status: Some(WatcherStatusInput {
                session_id: "s".to_string(),
                action: "triggered".to_string(),
                idle_since_secs: None,
            }),
            signals: vec![
                signal("s1", "watcher_trigger", "M", 3.0, None),
                signal("s2", "other", "L", 4.0, None),
            ],
        })
        .await;
    // 2 high (perm, question), 2 medium (watcher + watcher_trigger signal), 1 low.
    assert_eq!(items.len(), 5);
    // High first.
    assert!(matches!(items[0].priority, InboxItemPriority::High));
    assert!(matches!(items[1].priority, InboxItemPriority::High));
    // Medium next.
    assert!(matches!(items[2].priority, InboxItemPriority::Medium));
    assert!(matches!(items[3].priority, InboxItemPriority::Medium));
    // Low last.
    assert!(matches!(items[4].priority, InboxItemPriority::Low));
    assert_eq!(items[4].id, "inbox-signal-s2");
}
