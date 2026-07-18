//! Generated coverage tests for `db/signals.rs`: limit, delete-missing.
use super::*;

fn sig(id: &str, created: f64) -> SignalInput {
    SignalInput {
        id: id.into(),
        kind: "k".into(),
        title: format!("t-{id}"),
        body: String::new(),
        created_at: created,
        session_id: None,
    }
}

#[test]
fn list_respects_limit_and_desc_order() {
    let db = Db::open_memory().unwrap();
    for i in 0..4 {
        db.insert_signal(&sig(&format!("s{i}"), i as f64));
    }
    let limited = db.list_signals(2);
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].id, "s3");
    assert_eq!(limited[1].id, "s2");
}

#[test]
fn delete_missing_signal_is_false() {
    let db = Db::open_memory().unwrap();
    assert!(!db.delete_signal_row("nope"));
}

#[test]
fn prune_on_empty_is_noop() {
    let db = Db::open_memory().unwrap();
    db.prune_signals(5);
    assert!(db.list_signals(10).is_empty());
}
