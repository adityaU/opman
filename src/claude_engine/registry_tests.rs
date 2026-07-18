use super::*;

fn entry(id: &str, dir: &str, created: u64) -> SessionEntry {
    SessionEntry {
        id: id.to_string(),
        title: format!("title-{id}"),
        directory: dir.to_string(),
        created,
        updated: created,
        ..Default::default()
    }
}

#[test]
fn load_missing_path_yields_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("does-not-exist.json");
    let reg = Registry::load(&path);
    assert!(reg.sessions.is_empty());
    assert!(reg.deleted.is_empty());
}

#[test]
fn load_malformed_json_yields_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reg.json");
    std::fs::write(&path, "{ this is not valid json ]").unwrap();
    let reg = Registry::load(&path);
    assert!(reg.sessions.is_empty());
}

#[test]
fn save_then_load_roundtrips_sessions_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    // Nested path exercises the create_dir_all branch in save().
    let path = dir.path().join("nested").join("reg.json");

    let mut reg = Registry::default();
    let mut e = entry("ses_1", "/proj", 100);
    e.claude_session_id = Some("uuid-a".into());
    e.lineage = vec!["uuid-old".into(), "uuid-a".into()];
    e.model = Some("opus".into());
    e.busy = true; // #[serde(skip)] — must NOT survive a round-trip
    e.subagent_pending = true; // #[serde(skip)]
    reg.sessions.insert(e.id.clone(), e);
    reg.deleted.insert("dead-uuid".into());
    reg.save(&path);

    let loaded = Registry::load(&path);
    let got = loaded.sessions.get("ses_1").expect("session persisted");
    assert_eq!(got.claude_session_id.as_deref(), Some("uuid-a"));
    assert_eq!(got.lineage, vec!["uuid-old".to_string(), "uuid-a".to_string()]);
    assert_eq!(got.model.as_deref(), Some("opus"));
    assert!(!got.busy, "busy is skipped in serde and defaults to false");
    assert!(!got.subagent_pending, "subagent_pending is skipped");
    assert!(loaded.deleted.contains("dead-uuid"));
}

#[test]
fn skipped_fields_are_absent_from_serialized_json() {
    let mut reg = Registry::default();
    let mut e = entry("ses_1", "/proj", 1);
    e.busy = true;
    e.subagent_pending = true;
    reg.sessions.insert(e.id.clone(), e);
    let json = serde_json::to_string(&reg).unwrap();
    assert!(!json.contains("busy"));
    assert!(!json.contains("subagent_pending"));
}

#[test]
fn for_directory_filters_and_sorts_newest_first() {
    let mut reg = Registry::default();
    reg.sessions.insert("a".into(), entry("a", "/proj", 10));
    reg.sessions.insert("b".into(), entry("b", "/proj", 30));
    reg.sessions.insert("c".into(), entry("c", "/other", 20));
    reg.sessions.insert("d".into(), entry("d", "/proj", 20));

    let v = reg.for_directory("/proj");
    let ids: Vec<&str> = v.iter().map(|s| s.id.as_str()).collect();
    // Only /proj sessions, sorted by created descending.
    assert_eq!(ids, vec!["b", "d", "a"]);

    assert!(reg.for_directory("/nope").is_empty());
}

#[test]
fn by_claude_uuid_empty_is_none() {
    let reg = Registry::default();
    assert!(reg.by_claude_uuid("").is_none());
}

#[test]
fn by_claude_uuid_matches_latest_then_falls_back_to_lineage() {
    let mut reg = Registry::default();
    let mut e = entry("ses_1", "/proj", 1);
    e.claude_session_id = Some("current-uuid".into());
    e.lineage = vec!["old-uuid".into(), "current-uuid".into()];
    reg.sessions.insert(e.id.clone(), e);

    // Direct match on the current claude_session_id.
    assert_eq!(reg.by_claude_uuid("current-uuid").map(|s| s.id.as_str()), Some("ses_1"));
    // Fallback: a superseded lineage uuid still resolves to the same session.
    assert_eq!(reg.by_claude_uuid("old-uuid").map(|s| s.id.as_str()), Some("ses_1"));
    // Unknown uuid resolves to nothing.
    assert!(reg.by_claude_uuid("ghost").is_none());
}

#[test]
fn session_entry_default_is_empty() {
    let e = SessionEntry::default();
    assert!(e.id.is_empty());
    assert!(e.claude_session_id.is_none());
    assert!(!e.busy);
    assert!(!e.is_subagent);
    assert!(e.lineage.is_empty());
}
