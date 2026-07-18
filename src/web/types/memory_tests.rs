use super::*;
use serde_json::json;

#[test]
fn memory_scope_serialize_snake_case() {
    assert_eq!(serde_json::to_value(MemoryScope::Global).unwrap(), "global");
    assert_eq!(
        serde_json::to_value(MemoryScope::Project).unwrap(),
        "project"
    );
    assert_eq!(
        serde_json::to_value(MemoryScope::Session).unwrap(),
        "session"
    );
}

#[test]
fn memory_scope_deserialize_snake_case() {
    let g: MemoryScope = serde_json::from_value(json!("global")).unwrap();
    assert!(matches!(g, MemoryScope::Global));
    let p: MemoryScope = serde_json::from_value(json!("project")).unwrap();
    assert!(matches!(p, MemoryScope::Project));
    let s: MemoryScope = serde_json::from_value(json!("session")).unwrap();
    assert!(matches!(s, MemoryScope::Session));
    // Debug + Clone coverage.
    assert!(format!("{:?}", s.clone()).contains("Session"));
}

#[test]
fn memory_scope_unknown_rejected() {
    assert!(serde_json::from_value::<MemoryScope>(json!("bogus")).is_err());
}

#[test]
fn personal_memory_item_roundtrip() {
    let item = PersonalMemoryItem {
        id: "m1".into(),
        label: "Fav".into(),
        content: "42".into(),
        scope: MemoryScope::Project,
        project_index: Some(3),
        session_id: Some("s1".into()),
        created_at: "c".into(),
        updated_at: "u".into(),
    };
    let v = serde_json::to_value(&item).unwrap();
    assert_eq!(v["id"], "m1");
    assert_eq!(v["scope"], "project");
    assert_eq!(v["project_index"], 3);
    let back: PersonalMemoryItem = serde_json::from_value(v).unwrap();
    assert_eq!(back.label, "Fav");
    assert_eq!(back.project_index, Some(3));
    assert!(format!("{:?}", back.clone()).contains("PersonalMemoryItem"));
}

#[test]
fn personal_memory_item_defaults() {
    let item: PersonalMemoryItem = serde_json::from_value(json!({
        "id": "m",
        "label": "l",
        "content": "c",
        "scope": "global",
        "created_at": "a",
        "updated_at": "b"
    }))
    .unwrap();
    assert!(item.project_index.is_none());
    assert!(item.session_id.is_none());
}

#[test]
fn create_personal_memory_request_full_and_defaults() {
    let full: CreatePersonalMemoryRequest = serde_json::from_value(json!({
        "label": "L",
        "content": "C",
        "scope": "session",
        "project_index": 1,
        "session_id": "s"
    }))
    .unwrap();
    assert_eq!(full.label, "L");
    assert_eq!(full.project_index, Some(1));
    assert!(matches!(full.scope, MemoryScope::Session));
    assert!(format!("{:?}", full.clone()).contains("CreatePersonalMemoryRequest"));

    let minimal: CreatePersonalMemoryRequest = serde_json::from_value(json!({
        "label": "L",
        "content": "C",
        "scope": "global"
    }))
    .unwrap();
    assert!(minimal.project_index.is_none());
    assert!(minimal.session_id.is_none());
}

#[test]
fn update_personal_memory_request_double_option() {
    // All absent -> all None (default).
    let empty: UpdatePersonalMemoryRequest = serde_json::from_value(json!({})).unwrap();
    assert!(empty.label.is_none());
    assert!(empty.project_index.is_none());
    assert!(empty.session_id.is_none());

    // Plain `Option<Option<T>>` + `#[serde(default)]` (no double_option
    // deserializer): a present `null` collapses to the outer None.
    let cleared: UpdatePersonalMemoryRequest = serde_json::from_value(json!({
        "project_index": null,
        "session_id": null
    }))
    .unwrap();
    assert_eq!(cleared.project_index, None);
    assert_eq!(cleared.session_id, None);

    // Explicit value -> Some(Some(..)).
    let set: UpdatePersonalMemoryRequest = serde_json::from_value(json!({
        "label": "new",
        "content": "body",
        "scope": "project",
        "project_index": 7,
        "session_id": "sX"
    }))
    .unwrap();
    assert_eq!(set.label.as_deref(), Some("new"));
    assert_eq!(set.content.as_deref(), Some("body"));
    assert!(matches!(set.scope, Some(MemoryScope::Project)));
    assert_eq!(set.project_index, Some(Some(7)));
    assert_eq!(set.session_id, Some(Some("sX".into())));
    assert!(format!("{:?}", set.clone()).contains("UpdatePersonalMemoryRequest"));
}

#[test]
fn personal_memory_list_response_serialize() {
    let resp = PersonalMemoryListResponse {
        memory: vec![PersonalMemoryItem {
            id: "1".into(),
            label: "l".into(),
            content: "c".into(),
            scope: MemoryScope::Global,
            project_index: None,
            session_id: None,
            created_at: "a".into(),
            updated_at: "b".into(),
        }],
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["memory"][0]["id"], "1");
    assert!(format!("{:?}", resp.clone()).contains("PersonalMemoryListResponse"));
}
