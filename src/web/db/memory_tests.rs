//! Generated coverage tests for `db/memory.rs`: update-row, scope conversions.
use super::*;

fn item(id: &str, scope: MemoryScope, updated: &str) -> PersonalMemoryItem {
    PersonalMemoryItem {
        id: id.into(),
        label: format!("l-{id}"),
        content: "c".into(),
        scope,
        project_index: Some(1),
        session_id: Some("s".into()),
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: updated.into(),
    }
}

#[test]
fn list_sorted_desc_and_all_scopes_roundtrip() {
    let db = Db::open_memory().unwrap();
    db.insert_memory(&item("g", MemoryScope::Global, "2025-01-01T00:00:00Z"));
    db.insert_memory(&item("p", MemoryScope::Project, "2025-03-01T00:00:00Z"));
    db.insert_memory(&item("s", MemoryScope::Session, "2025-02-01T00:00:00Z"));
    let list = db.list_memory();
    assert_eq!(list.iter().map(|m| m.id.clone()).collect::<Vec<_>>(), vec!["p", "s", "g"]);
    assert!(matches!(list[0].scope, MemoryScope::Project));
    assert!(matches!(list[1].scope, MemoryScope::Session));
    assert!(matches!(list[2].scope, MemoryScope::Global));
}

#[test]
fn update_memory_row_found_and_not_found() {
    let db = Db::open_memory().unwrap();
    let mut m = item("u1", MemoryScope::Global, "2025-01-01T00:00:00Z");
    db.insert_memory(&m);

    m.label = "renamed".into();
    m.content = "new".into();
    m.scope = MemoryScope::Session;
    m.updated_at = "2025-02-01T00:00:00Z".into();
    assert!(db.update_memory_row(&m));
    let got = &db.list_memory()[0];
    assert_eq!(got.label, "renamed");
    assert!(matches!(got.scope, MemoryScope::Session));

    assert!(!db.update_memory_row(&item("ghost", MemoryScope::Global, "2025-01-01T00:00:00Z")));
}

#[test]
fn delete_memory_row_missing_is_false() {
    let db = Db::open_memory().unwrap();
    assert!(!db.delete_memory_row("nope"));
}

#[test]
fn scope_conversions() {
    assert_eq!(memory_scope_str(&MemoryScope::Global), "global");
    assert_eq!(memory_scope_str(&MemoryScope::Project), "project");
    assert_eq!(memory_scope_str(&MemoryScope::Session), "session");
    assert!(matches!(parse_memory_scope("project"), MemoryScope::Project));
    assert!(matches!(parse_memory_scope("session"), MemoryScope::Session));
    assert!(matches!(parse_memory_scope("global"), MemoryScope::Global));
    assert!(matches!(parse_memory_scope("other"), MemoryScope::Global));
}
