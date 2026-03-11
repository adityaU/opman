//! Signal CRUD backed by SQLite.

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// List all signals, sorted by created_at DESC, limited to `limit`.
    pub fn list_signals(&self, limit: usize) -> Vec<SignalInput> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, title, body, created_at, session_id
                 FROM signals ORDER BY created_at DESC LIMIT ?1",
            )
            .expect("prepare list_signals");

        stmt.query_map(params![limit as i64], |row| {
            Ok(SignalInput {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
                session_id: row.get(5)?,
            })
        })
        .expect("query list_signals")
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Insert a signal.
    pub fn insert_signal(&self, s: &SignalInput) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO signals (id, kind, title, body, created_at, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![s.id, s.kind, s.title, s.body, s.created_at, s.session_id],
        )
        .expect("insert_signal");
    }

    /// Delete a signal by ID. Returns true if it existed.
    #[allow(dead_code)]
    pub fn delete_signal_row(&self, id: &str) -> bool {
        let conn = self.conn();
        let changed = conn
            .execute("DELETE FROM signals WHERE id=?1", params![id])
            .expect("delete_signal");
        changed > 0
    }

    /// Prune signals to keep only the newest `max_count`.
    #[allow(dead_code)]
    pub fn prune_signals(&self, max_count: usize) {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM signals WHERE id NOT IN
                (SELECT id FROM signals ORDER BY created_at DESC LIMIT ?1)",
            params![max_count as i64],
        )
        .expect("prune_signals");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_round_trip() {
        let db = Db::open_memory().unwrap();
        let s = SignalInput {
            id: "sig-1".into(),
            kind: "cost_alert".into(),
            title: "High cost".into(),
            body: "Session exceeded $5".into(),
            created_at: 1700000000.0,
            session_id: Some("sess-1".into()),
        };
        db.insert_signal(&s);
        let list = db.list_signals(100);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "High cost");
        assert!(db.delete_signal_row("sig-1"));
        assert!(db.list_signals(100).is_empty());
    }

    #[test]
    fn prune_keeps_newest() {
        let db = Db::open_memory().unwrap();
        for i in 0..5 {
            db.insert_signal(&SignalInput {
                id: format!("sig-{i}"),
                kind: "test".into(),
                title: format!("Signal {i}"),
                body: String::new(),
                created_at: i as f64,
                session_id: None,
            });
        }
        assert_eq!(db.list_signals(100).len(), 5);
        db.prune_signals(2);
        let remaining = db.list_signals(100);
        assert_eq!(remaining.len(), 2);
        // Newest first
        assert_eq!(remaining[0].id, "sig-4");
        assert_eq!(remaining[1].id, "sig-3");
    }
}
