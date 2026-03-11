//! Mission CRUD backed by SQLite.

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// List all missions, sorted by updated_at DESC.
    pub fn list_missions(&self) -> Vec<Mission> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, goal, next_action, status,
                        project_index, session_id, created_at, updated_at
                 FROM missions ORDER BY updated_at DESC",
            )
            .expect("prepare list_missions");

        stmt.query_map([], |row| {
            Ok(Mission {
                id: row.get(0)?,
                title: row.get(1)?,
                goal: row.get(2)?,
                next_action: row.get(3)?,
                status: parse_mission_status(&row.get::<_, String>(4)?),
                project_index: row.get::<_, i64>(5)? as usize,
                session_id: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .expect("query list_missions")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a new mission.
    pub fn insert_mission(&self, m: &Mission) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO missions
                (id, title, goal, next_action, status,
                 project_index, session_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                m.id,
                m.title,
                m.goal,
                m.next_action,
                mission_status_str(&m.status),
                m.project_index as i64,
                m.session_id,
                m.created_at,
                m.updated_at,
            ],
        )
        .expect("insert_mission");
    }

    /// Update a mission by ID. Returns true if the row existed.
    #[allow(dead_code)]
    pub fn update_mission_row(&self, m: &Mission) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute(
                "UPDATE missions SET title=?1, goal=?2, next_action=?3,
                    status=?4, project_index=?5, session_id=?6, updated_at=?7
                 WHERE id=?8",
                params![
                    m.title,
                    m.goal,
                    m.next_action,
                    mission_status_str(&m.status),
                    m.project_index as i64,
                    m.session_id,
                    m.updated_at,
                    m.id,
                ],
            )
            .expect("update_mission");
        changed > 0
    }

    /// Delete a mission by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_mission_row(&self, id: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM missions WHERE id=?1", params![id])
            .expect("delete_mission");
        changed > 0
    }
}

fn mission_status_str(s: &MissionStatus) -> &'static str {
    match s {
        MissionStatus::Planned => "planned",
        MissionStatus::Active => "active",
        MissionStatus::Blocked => "blocked",
        MissionStatus::Completed => "completed",
    }
}

fn parse_mission_status(s: &str) -> MissionStatus {
    match s {
        "active" => MissionStatus::Active,
        "blocked" => MissionStatus::Blocked,
        "completed" => MissionStatus::Completed,
        _ => MissionStatus::Planned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_round_trip() {
        let db = Db::open_memory().unwrap();
        let m = Mission {
            id: "m-1".into(),
            title: "Ship v2".into(),
            goal: "Release".into(),
            next_action: "Write tests".into(),
            status: MissionStatus::Active,
            project_index: 0,
            session_id: Some("sess-1".into()),
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
        };
        db.insert_mission(&m);
        let list = db.list_missions();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Ship v2");
        assert!(db.delete_mission_row("m-1"));
        assert!(db.list_missions().is_empty());
    }
}
