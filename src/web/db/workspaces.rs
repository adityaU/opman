//! Workspace snapshot CRUD backed by SQLite.
//!
//! Workspaces are stored as a JSON blob in the `snapshot` column since
//! they contain deeply nested optional fields (panels, layout, terminal
//! tabs, recipe metadata) that would create excessive columns.

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// List all workspace snapshots, sorted by created_at DESC.
    pub fn list_workspaces(&self) -> Vec<WorkspaceSnapshot> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT snapshot FROM workspaces ORDER BY created_at DESC")
            .expect("prepare list_workspaces");

        stmt.query_map([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        })
        .expect("query list_workspaces")
        .filter_map(|r| r.ok())
        .filter_map(|json| serde_json::from_str::<WorkspaceSnapshot>(&json).ok())
        .collect()
    }

    /// Upsert a workspace snapshot (keyed by name).
    pub fn upsert_workspace(&self, ws: &WorkspaceSnapshot) {
        let json = serde_json::to_string(ws).expect("serialize workspace");
        let conn = self.conn();
        conn.execute(
            "INSERT INTO workspaces (name, snapshot, created_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET snapshot=excluded.snapshot",
            params![ws.name, json, ws.created_at],
        )
        .expect("upsert_workspace");
    }

    /// Delete a workspace by name. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_workspace_row(&self, name: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM workspaces WHERE name=?1", params![name])
            .expect("delete_workspace");
        changed > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_round_trip() {
        let db = Db::open_memory().unwrap();
        let ws = WorkspaceSnapshot {
            name: "coding".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            panels: WorkspacePanels {
                sidebar: true,
                terminal: true,
                editor: true,
                git: false,
            },
            layout: WorkspaceLayout::default(),
            open_files: vec!["main.rs".into()],
            active_file: Some("main.rs".into()),
            terminal_tabs: vec![],
            session_id: None,
            git_branch: None,
            is_template: false,
            recipe_description: None,
            recipe_next_action: None,
            is_recipe: false,
        };
        db.upsert_workspace(&ws);
        let list = db.list_workspaces();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "coding");
        assert!(db.delete_workspace_row("coding"));
        assert!(db.list_workspaces().is_empty());
    }
}
