//! Tests for personal memory CRUD.
use super::*;
use crate::web::web_state::WebStateHandle;

// ── Personal Memory ─────────────────────────────────────────────────

fn mk_create_memory(label: &str) -> CreatePersonalMemoryRequest {
    CreatePersonalMemoryRequest {
        label: label.to_string(),
        content: "content".to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    }
}

#[tokio::test]
async fn personal_memory_crud_lifecycle() {
    let h = WebStateHandle::new_test();
    assert!(h.list_personal_memory().await.is_empty());

    let item = h.create_personal_memory(mk_create_memory("first")).await;
    assert!(item.id.starts_with("memory-"));
    assert_eq!(item.label, "first");
    assert_eq!(h.list_personal_memory().await.len(), 1);

    // Update all fields.
    let upd = UpdatePersonalMemoryRequest {
        label: Some("renamed".to_string()),
        content: Some("new content".to_string()),
        scope: Some(MemoryScope::Project),
        project_index: Some(Some(2)),
        session_id: Some(Some("s".to_string())),
    };
    let updated = h.update_personal_memory(&item.id, upd).await.unwrap();
    assert_eq!(updated.label, "renamed");
    assert_eq!(updated.content, "new content");
    assert!(matches!(updated.scope, MemoryScope::Project));
    assert_eq!(updated.project_index, Some(2));
    assert_eq!(updated.session_id.as_deref(), Some("s"));

    assert!(h.delete_personal_memory(&item.id).await);
    assert!(!h.delete_personal_memory(&item.id).await);
}

#[tokio::test]
async fn personal_memory_update_not_found_and_no_fields() {
    let h = WebStateHandle::new_test();
    let none_req = UpdatePersonalMemoryRequest {
        label: None,
        content: None,
        scope: None,
        project_index: None,
        session_id: None,
    };
    assert!(h
        .update_personal_memory("missing", none_req)
        .await
        .is_none());

    let item = h.create_personal_memory(mk_create_memory("x")).await;
    let none_req2 = UpdatePersonalMemoryRequest {
        label: None,
        content: None,
        scope: None,
        project_index: None,
        session_id: None,
    };
    let unchanged = h.update_personal_memory(&item.id, none_req2).await.unwrap();
    assert_eq!(unchanged.label, "x");
}

#[tokio::test]
async fn personal_memory_list_sorted_by_updated_desc() {
    let h = WebStateHandle::new_test();
    let a = h.create_personal_memory(mk_create_memory("a")).await;
    let _b = h.create_personal_memory(mk_create_memory("b")).await;
    // Force a's updated_at to be newer.
    {
        let mut s = h.inner.write().await;
        s.personal_memory.get_mut(&a.id).unwrap().updated_at = "2999-01-01T00:00:00Z".to_string();
    }
    let list = h.list_personal_memory().await;
    assert_eq!(list[0].id, a.id);
}
