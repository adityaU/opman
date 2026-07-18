use super::*;
use crate::web::types::*;
use crate::web::web_state::WebStateHandle;

fn create_req(title: &str) -> CreateDelegatedWorkRequest {
    CreateDelegatedWorkRequest {
        title: title.to_string(),
        assignee: "agent".to_string(),
        scope: "scope".to_string(),
        mission_id: Some("m1".to_string()),
        session_id: Some("sess".to_string()),
        subagent_session_id: Some("sub".to_string()),
    }
}

fn workspace(name: &str, created_at: &str) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        name: name.to_string(),
        created_at: created_at.to_string(),
        panels: WorkspacePanels { sidebar: true, terminal: false, editor: true, git: false },
        layout: WorkspaceLayout::default(),
        open_files: vec!["a.rs".to_string()],
        active_file: Some("a.rs".to_string()),
        terminal_tabs: vec![],
        session_id: Some("s".to_string()),
        git_branch: Some("main".to_string()),
        is_template: false,
        recipe_description: None,
        recipe_next_action: None,
        is_recipe: false,
    }
}

// ── delegated work ───────────────────────────────────────────────────

#[tokio::test]
async fn delegated_work_empty() {
    let h = WebStateHandle::new_test();
    assert!(h.list_delegated_work().await.is_empty());
}

#[tokio::test]
async fn delegated_work_create_and_list() {
    let h = WebStateHandle::new_test();
    let item = h.create_delegated_work(create_req("task")).await;
    assert!(item.id.starts_with("delegation-"));
    assert_eq!(item.title, "task");
    assert_eq!(item.assignee, "agent");
    assert!(matches!(item.status, DelegationStatus::Planned));
    assert_eq!(item.mission_id.as_deref(), Some("m1"));
    assert_eq!(item.session_id.as_deref(), Some("sess"));
    assert_eq!(item.subagent_session_id.as_deref(), Some("sub"));

    let list = h.list_delegated_work().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, item.id);
}

#[tokio::test]
async fn delegated_work_list_sorted_by_updated_desc() {
    let h = WebStateHandle::new_test();
    let a = h.create_delegated_work(create_req("a")).await;
    // Force a later updated_at on the second item.
    let b = h.create_delegated_work(create_req("b")).await;
    // Bump b's updated_at explicitly via update.
    let b_updated = h
        .update_delegated_work(&b.id, UpdateDelegatedWorkRequest { status: Some(DelegationStatus::Running) })
        .await
        .unwrap();
    let list = h.list_delegated_work().await;
    // b was updated most recently → sorts first (unless timestamps collide,
    // in which case both are present).
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|d| d.id == a.id));
    assert!(list.iter().any(|d| d.id == b_updated.id));
    assert!(list[0].updated_at >= list[1].updated_at);
}

#[tokio::test]
async fn delegated_work_update_status_and_no_status() {
    let h = WebStateHandle::new_test();
    let item = h.create_delegated_work(create_req("task")).await;

    // Update with a status.
    let updated = h
        .update_delegated_work(&item.id, UpdateDelegatedWorkRequest { status: Some(DelegationStatus::Completed) })
        .await
        .unwrap();
    assert!(matches!(updated.status, DelegationStatus::Completed));

    // Update with no status → keeps status, refreshes updated_at.
    let again = h
        .update_delegated_work(&item.id, UpdateDelegatedWorkRequest { status: None })
        .await
        .unwrap();
    assert!(matches!(again.status, DelegationStatus::Completed));
}

#[tokio::test]
async fn delegated_work_update_missing_returns_none() {
    let h = WebStateHandle::new_test();
    let out = h
        .update_delegated_work("nope", UpdateDelegatedWorkRequest { status: Some(DelegationStatus::Running) })
        .await;
    assert!(out.is_none());
}

#[tokio::test]
async fn delegated_work_delete() {
    let h = WebStateHandle::new_test();
    let item = h.create_delegated_work(create_req("task")).await;
    assert!(h.delete_delegated_work(&item.id).await);
    // Second delete → false.
    assert!(!h.delete_delegated_work(&item.id).await);
    assert!(h.list_delegated_work().await.is_empty());
}

// ── workspace snapshots ──────────────────────────────────────────────

#[tokio::test]
async fn workspaces_empty() {
    let h = WebStateHandle::new_test();
    assert!(h.list_workspaces().await.is_empty());
}

#[tokio::test]
async fn workspaces_save_list_sorted_and_upsert() {
    let h = WebStateHandle::new_test();
    h.save_workspace(workspace("old", "2024-01-01T00:00:00Z")).await;
    h.save_workspace(workspace("new", "2024-06-01T00:00:00Z")).await;
    let list = h.list_workspaces().await;
    assert_eq!(list.len(), 2);
    // Newest created_at first.
    assert_eq!(list[0].name, "new");
    assert_eq!(list[1].name, "old");

    // Upsert: same name replaces.
    let mut updated = workspace("old", "2025-01-01T00:00:00Z");
    updated.git_branch = Some("dev".to_string());
    h.save_workspace(updated).await;
    let list = h.list_workspaces().await;
    assert_eq!(list.len(), 2);
    let old = list.iter().find(|w| w.name == "old").unwrap();
    assert_eq!(old.git_branch.as_deref(), Some("dev"));
    assert_eq!(old.created_at, "2025-01-01T00:00:00Z");
}

#[tokio::test]
async fn workspaces_delete() {
    let h = WebStateHandle::new_test();
    h.save_workspace(workspace("ws", "2024-01-01T00:00:00Z")).await;
    assert!(h.delete_workspace("ws").await);
    assert!(!h.delete_workspace("ws").await);
    assert!(h.list_workspaces().await.is_empty());
}
