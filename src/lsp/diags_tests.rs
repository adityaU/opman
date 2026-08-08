//! The empty-array case is the one worth guarding: treating "clean" as
//! "unknown" leaves a fixed file showing its old errors indefinitely.

use super::*;
use serde_json::json;

fn diag(message: &str) -> Value {
    json!({ "message": message, "range": { "start": { "line": 0, "character": 0 } } })
}

#[test]
fn unknown_and_clean_are_different_answers() {
    let store = DiagStore::new();
    assert!(store.get("file:///a.rs").is_none(), "never heard of it");

    store.publish("file:///a.rs".into(), vec![]);
    assert_eq!(
        store.get("file:///a.rs"),
        Some(vec![]),
        "an empty publish means the file is clean, not unknown"
    );
}

#[test]
fn a_publish_replaces_the_previous_set() {
    let store = DiagStore::new();
    store.publish("file:///a.rs".into(), vec![diag("first"), diag("second")]);
    assert_eq!(store.get("file:///a.rs").unwrap().len(), 2);

    store.publish("file:///a.rs".into(), vec![diag("only")]);
    let found = store.get("file:///a.rs").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["message"], "only");
}

#[test]
fn files_do_not_share_diagnostics() {
    let store = DiagStore::new();
    store.publish("file:///a.rs".into(), vec![diag("a")]);
    store.publish("file:///b.rs".into(), vec![]);
    assert_eq!(store.get("file:///a.rs").unwrap().len(), 1);
    assert_eq!(store.get("file:///b.rs").unwrap().len(), 0);
}

#[tokio::test]
async fn waiting_returns_immediately_when_already_known() {
    let store = DiagStore::new();
    store.publish("file:///a.rs".into(), vec![diag("known")]);

    let start = std::time::Instant::now();
    let found = store.wait_for("file:///a.rs", Duration::from_secs(5)).await;
    assert_eq!(found.len(), 1);
    assert!(
        start.elapsed() < Duration::from_millis(200),
        "should not wait"
    );
}

#[tokio::test]
async fn waiting_wakes_on_a_later_publish() {
    let store = Arc::new(DiagStore::new());
    let writer = store.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        writer.publish("file:///a.rs".into(), vec![diag("late")]);
    });

    let found = store.wait_for("file:///a.rs", Duration::from_secs(5)).await;
    assert_eq!(found[0]["message"], "late");
}

/// A cold server may say nothing for a minute; the request must still return.
#[tokio::test]
async fn waiting_gives_up_and_reports_clean() {
    let store = DiagStore::new();
    let found = store
        .wait_for("file:///silent.rs", Duration::from_millis(80))
        .await;
    assert!(found.is_empty());
}

#[test]
fn clearing_forgets_everything() {
    let store = DiagStore::new();
    store.publish("file:///a.rs".into(), vec![diag("a")]);
    store.clear();
    assert!(store.get("file:///a.rs").is_none());
}

use std::sync::Arc;
