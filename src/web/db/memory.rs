//! Personal memory CRUD backed by SQLite.

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// List all personal memory items, sorted by updated_at DESC.
    pub fn list_memory(&self) -> Vec<PersonalMemoryItem> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, label, content, scope, project_index,
                        session_id, created_at, updated_at
                 FROM personal_memory ORDER BY updated_at DESC",
            )
            .expect("prepare list_memory");

        stmt.query_map([], |row| {
            Ok(PersonalMemoryItem {
                id: row.get(0)?,
                label: row.get(1)?,
                content: row.get(2)?,
                scope: parse_memory_scope(&row.get::<_, String>(3)?),
                project_index: row.get::<_, Option<i64>>(4)?.map(|v| v as usize),
                session_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .expect("query list_memory")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a personal memory item.
    pub fn insert_memory(&self, m: &PersonalMemoryItem) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO personal_memory
                (id, label, content, scope, project_index,
                 session_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                m.id,
                m.label,
                m.content,
                memory_scope_str(&m.scope),
                m.project_index.map(|v| v as i64),
                m.session_id,
                m.created_at,
                m.updated_at,
            ],
        )
        .expect("insert_memory");
    }

    /// Update a memory item by ID. Returns true if the row existed.
    #[allow(dead_code)]
    pub fn update_memory_row(&self, m: &PersonalMemoryItem) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute(
                "UPDATE personal_memory SET label=?1, content=?2, scope=?3,
                    project_index=?4, session_id=?5, updated_at=?6
                 WHERE id=?7",
                params![
                    m.label,
                    m.content,
                    memory_scope_str(&m.scope),
                    m.project_index.map(|v| v as i64),
                    m.session_id,
                    m.updated_at,
                    m.id,
                ],
            )
            .expect("update_memory");
        changed > 0
    }

    /// Delete a memory item by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_memory_row(&self, id: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM personal_memory WHERE id=?1", params![id])
            .expect("delete_memory");
        changed > 0
    }
}

fn memory_scope_str(s: &MemoryScope) -> &'static str {
    match s {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
        MemoryScope::Session => "session",
    }
}

fn parse_memory_scope(s: &str) -> MemoryScope {
    match s {
        "project" => MemoryScope::Project,
        "session" => MemoryScope::Session,
        _ => MemoryScope::Global,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_round_trip() {
        let db = Db::open_memory().unwrap();
        let m = PersonalMemoryItem {
            id: "mem-1".into(),
            label: "API key pattern".into(),
            content: "Use env vars".into(),
            scope: MemoryScope::Global,
            project_index: None,
            session_id: None,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_memory(&m);
        let list = db.list_memory();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "API key pattern");
        assert!(db.delete_memory_row("mem-1"));
        assert!(db.list_memory().is_empty());
    }
}
