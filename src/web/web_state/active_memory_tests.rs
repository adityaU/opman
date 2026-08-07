use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

// ── list_active_memory ──────────────────────────────────────────

#[tokio::test]
async fn active_memory_filters_by_scope() {
    let h = WebStateHandle::new_test();
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "global".to_string(),
        content: "c".to_string(),
        scope: MemoryScope::Global,
        project_index: None,
        session_id: None,
    })
    .await;
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "proj".to_string(),
        content: "c".to_string(),
        scope: MemoryScope::Project,
        project_index: Some(3),
        session_id: None,
    })
    .await;
    h.create_personal_memory(CreatePersonalMemoryRequest {
        label: "sess".to_string(),
        content: "c".to_string(),
        scope: MemoryScope::Session,
        project_index: None,
        session_id: Some("sid-1".to_string()),
    })
    .await;

    // No project / no session: only Global visible.
    let only_global = h.list_active_memory(None, None).await;
    assert_eq!(only_global.len(), 1);
    assert_eq!(only_global[0].label, "global");

    // Matching project index → Global + Project.
    let with_proj = h.list_active_memory(Some(3), None).await;
    let labels: Vec<&str> = with_proj.iter().map(|m| m.label.as_str()).collect();
    assert!(labels.contains(&"global"));
    assert!(labels.contains(&"proj"));
    assert!(!labels.contains(&"sess"));

    // Non-matching project index → Project filtered out.
    let wrong_proj = h.list_active_memory(Some(99), None).await;
    assert_eq!(wrong_proj.len(), 1);
    assert_eq!(wrong_proj[0].label, "global");

    // Matching session id → Global + Session.
    let with_sess = h.list_active_memory(None, Some("sid-1")).await;
    let labels: Vec<&str> = with_sess.iter().map(|m| m.label.as_str()).collect();
    assert!(labels.contains(&"global"));
    assert!(labels.contains(&"sess"));
    assert!(!labels.contains(&"proj"));

    // Non-matching session id → Session filtered out.
    let wrong_sess = h.list_active_memory(None, Some("other")).await;
    assert_eq!(wrong_sess.len(), 1);
    assert_eq!(wrong_sess[0].label, "global");
}
