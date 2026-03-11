//! Delegated work CRUD backed by SQLite.

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// List all delegated work items, sorted by updated_at DESC.
    pub fn list_delegated_work(&self) -> Vec<DelegatedWorkItem> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, assignee, scope, status,
                        mission_id, session_id, subagent_session_id,
                        created_at, updated_at
                 FROM delegated_work ORDER BY updated_at DESC",
            )
            .expect("prepare list_delegated_work");

        stmt.query_map([], |row| {
            Ok(DelegatedWorkItem {
                id: row.get(0)?,
                title: row.get(1)?,
                assignee: row.get(2)?,
                scope: row.get(3)?,
                status: parse_delegation_status(&row.get::<_, String>(4)?),
                mission_id: row.get(5)?,
                session_id: row.get(6)?,
                subagent_session_id: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .expect("query list_delegated_work")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a delegated work item.
    pub fn insert_delegated_work(&self, d: &DelegatedWorkItem) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO delegated_work
                (id, title, assignee, scope, status,
                 mission_id, session_id, subagent_session_id,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                d.id,
                d.title,
                d.assignee,
                d.scope,
                delegation_status_str(&d.status),
                d.mission_id,
                d.session_id,
                d.subagent_session_id,
                d.created_at,
                d.updated_at,
            ],
        )
        .expect("insert_delegated_work");
    }

    /// Update a delegated work item by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn update_delegated_work_row(&self, d: &DelegatedWorkItem) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute(
                "UPDATE delegated_work SET title=?1, assignee=?2, scope=?3,
                    status=?4, mission_id=?5, session_id=?6,
                    subagent_session_id=?7, updated_at=?8
                 WHERE id=?9",
                params![
                    d.title,
                    d.assignee,
                    d.scope,
                    delegation_status_str(&d.status),
                    d.mission_id,
                    d.session_id,
                    d.subagent_session_id,
                    d.updated_at,
                    d.id,
                ],
            )
            .expect("update_delegated_work");
        changed > 0
    }

    /// Delete a delegated work item by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_delegated_work_row(&self, id: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM delegated_work WHERE id=?1", params![id])
            .expect("delete_delegated_work");
        changed > 0
    }
}

fn delegation_status_str(s: &DelegationStatus) -> &'static str {
    match s {
        DelegationStatus::Planned => "planned",
        DelegationStatus::Running => "running",
        DelegationStatus::Completed => "completed",
    }
}

fn parse_delegation_status(s: &str) -> DelegationStatus {
    match s {
        "running" => DelegationStatus::Running,
        "completed" => DelegationStatus::Completed,
        _ => DelegationStatus::Planned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_round_trip() {
        let db = Db::open_memory().unwrap();
        let d = DelegatedWorkItem {
            id: "dw-1".into(),
            title: "Run linter".into(),
            assignee: "subagent-A".into(),
            scope: "lint".into(),
            status: DelegationStatus::Running,
            mission_id: None,
            session_id: None,
            subagent_session_id: Some("sub-sess-1".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_delegated_work(&d);
        let list = db.list_delegated_work();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Run linter");
        assert!(db.delete_delegated_work_row("dw-1"));
        assert!(db.list_delegated_work().is_empty());
    }
}
