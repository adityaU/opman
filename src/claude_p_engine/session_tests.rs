use super::*;

fn tmp_path(tag: &str) -> PathBuf {
    let n: u128 = rand::random();
    std::env::temp_dir().join(format!("opman_p_sess_{tag}_{n:032x}.json"))
}

fn sample(id: &str, dir: &str) -> Session {
    Session {
        id: id.to_string(),
        title: "Title".into(),
        directory: dir.to_string(),
        parent_id: "parent".into(),
        created: 111,
        updated: 222,
        claude_uuid: Some("uuid-1".into()),
        model: Some("m".into()),
        agent: Some("a".into()),
        effort: Some("high".into()),
        permission_mode: Some("plan".into()),
        allowed_tools: vec!["Bash".into()],
        busy: true,
        title_locked: true,
        is_subagent: false,
    }
}

#[test]
fn session_info_shape() {
    let s = sample("ses_1", "/d");
    let v = session_info(&s);
    assert_eq!(v["id"], "ses_1");
    assert_eq!(v["title"], "Title");
    assert_eq!(v["projectID"], "claude");
    assert_eq!(v["parentID"], "parent");
    assert_eq!(v["directory"], "/d");
    assert_eq!(v["time"]["created"], 111);
    assert_eq!(v["time"]["updated"], 222);
    assert_eq!(v["slug"], "");
}

#[test]
fn persist_from_session_copies_fields() {
    let s = sample("ses_2", "/d2");
    let p = PersistSession::from(&s);
    assert_eq!(p.id, "ses_2");
    assert_eq!(p.claude_uuid.as_deref(), Some("uuid-1"));
    assert_eq!(p.model.as_deref(), Some("m"));
    assert_eq!(p.agent.as_deref(), Some("a"));
    assert_eq!(p.permission_mode.as_deref(), Some("plan"));
    assert!(p.title_locked);
}

#[test]
fn save_none_persist_is_noop() {
    let mut map = HashMap::new();
    map.insert("a".to_string(), sample("a", "/d"));
    save_sessions(&None, &map); // must not panic
}

#[test]
fn save_and_load_roundtrip_filters_subagents() {
    let path = tmp_path("rt");
    let mut map = HashMap::new();
    map.insert("real".to_string(), sample("real", "/d"));
    let mut sub = sample("sub", "/d");
    sub.is_subagent = true;
    map.insert("sub".to_string(), sub);

    save_sessions(&Some(path.clone()), &map);
    let loaded = load_sessions(&Some(path.clone()));

    assert!(loaded.contains_key("real"));
    assert!(
        !loaded.contains_key("sub"),
        "subagent rows are not persisted"
    );
    let r = &loaded["real"];
    assert_eq!(r.title, "Title");
    assert_eq!(r.claude_uuid.as_deref(), Some("uuid-1"));
    assert_eq!(r.permission_mode.as_deref(), Some("plan"));
    assert!(r.title_locked);
    // Live-only fields reset on load.
    assert!(!r.busy);
    assert!(r.allowed_tools.is_empty());
    assert!(!r.is_subagent);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn load_missing_file_is_empty() {
    let path = tmp_path("missing");
    let loaded = load_sessions(&Some(path));
    assert!(loaded.is_empty());
}

#[test]
fn load_none_persist_is_empty() {
    assert!(load_sessions(&None).is_empty());
}

#[test]
fn load_bad_json_is_empty() {
    let path = tmp_path("bad");
    std::fs::write(&path, "not json at all").unwrap();
    let loaded = load_sessions(&Some(path.clone()));
    assert!(loaded.is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn save_creates_parent_dir() {
    let n: u128 = rand::random();
    let dir = std::env::temp_dir().join(format!("opman_p_nested_{n:032x}"));
    let path = dir.join("sub").join("sessions.json");
    let mut map = HashMap::new();
    map.insert("a".to_string(), sample("a", "/d"));
    save_sessions(&Some(path.clone()), &map);
    assert!(path.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
