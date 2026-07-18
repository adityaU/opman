//! Generated coverage tests for `db/workspaces.rs`: upsert-conflict, ordering,
//! delete-missing, and malformed-snapshot skipping.
use super::*;

fn ws(name: &str, created: &str) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        name: name.into(),
        created_at: created.into(),
        panels: WorkspacePanels {
            sidebar: true,
            terminal: true,
            editor: true,
            git: false,
        },
        layout: WorkspaceLayout::default(),
        open_files: vec![],
        active_file: None,
        terminal_tabs: vec![],
        session_id: None,
        git_branch: None,
        is_template: false,
        recipe_description: None,
        recipe_next_action: None,
        is_recipe: false,
    }
}

#[test]
fn upsert_conflict_updates_snapshot_in_place() {
    let db = Db::open_memory().unwrap();
    let mut w = ws("dev", "2025-01-01T00:00:00Z");
    db.upsert_workspace(&w);
    w.open_files = vec!["a.rs".into(), "b.rs".into()];
    db.upsert_workspace(&w); // same name → ON CONFLICT UPDATE

    let list = db.list_workspaces();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].open_files.len(), 2);
}

#[test]
fn list_sorted_by_created_desc() {
    let db = Db::open_memory().unwrap();
    db.upsert_workspace(&ws("old", "2025-01-01T00:00:00Z"));
    db.upsert_workspace(&ws("new", "2025-06-01T00:00:00Z"));
    let list = db.list_workspaces();
    assert_eq!(list[0].name, "new");
    assert_eq!(list[1].name, "old");
}

#[test]
fn delete_missing_workspace_is_false() {
    let db = Db::open_memory().unwrap();
    assert!(!db.delete_workspace_row("nope"));
}

#[test]
fn malformed_snapshot_rows_are_skipped() {
    let db = Db::open_memory().unwrap();
    db.upsert_workspace(&ws("good", "2025-01-01T00:00:00Z"));
    // Insert a row whose snapshot JSON cannot deserialize into WorkspaceSnapshot.
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO workspaces (name, snapshot, created_at) VALUES ('bad', '{not json', '2025-02-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }
    let list = db.list_workspaces();
    // Only the good one survives deserialization.
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "good");
}
