//! Autonomy settings CRUD backed by SQLite.
//!
//! The `autonomy_settings` table has a single row (id=1).

use rusqlite::params;

use super::Db;
use crate::web::types::*;

impl Db {
    /// Load the current autonomy settings, or return default if none saved.
    pub fn load_autonomy_settings(&self) -> AutonomySettings {
        let conn = self.conn();
        conn.query_row(
            "SELECT mode, updated_at FROM autonomy_settings WHERE id=1",
            [],
            |row| {
                Ok(AutonomySettings {
                    mode: parse_autonomy_mode(&row.get::<_, String>(0)?),
                    updated_at: row.get(1)?,
                })
            },
        )
        .unwrap_or_else(|_| AutonomySettings {
            mode: AutonomyMode::Observe,
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Save (upsert) autonomy settings.
    pub fn save_autonomy_settings(&self, s: &AutonomySettings) {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO autonomy_settings (id, mode, updated_at)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET mode=excluded.mode, updated_at=excluded.updated_at",
            params![autonomy_mode_str(&s.mode), s.updated_at],
        )
        .expect("save_autonomy_settings");
    }
}

fn autonomy_mode_str(m: &AutonomyMode) -> &'static str {
    match m {
        AutonomyMode::Observe => "observe",
        AutonomyMode::Nudge => "nudge",
        AutonomyMode::Continue => "continue",
        AutonomyMode::Autonomous => "autonomous",
    }
}

fn parse_autonomy_mode(s: &str) -> AutonomyMode {
    match s {
        "nudge" => AutonomyMode::Nudge,
        "continue" => AutonomyMode::Continue,
        "autonomous" => AutonomyMode::Autonomous,
        _ => AutonomyMode::Observe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let db = Db::open_memory().unwrap();

        // Default when nothing saved
        let s = db.load_autonomy_settings();
        assert!(matches!(s.mode, AutonomyMode::Observe));

        // Save and reload
        let updated = AutonomySettings {
            mode: AutonomyMode::Autonomous,
            updated_at: "2025-06-01T00:00:00Z".into(),
        };
        db.save_autonomy_settings(&updated);
        let loaded = db.load_autonomy_settings();
        assert!(matches!(loaded.mode, AutonomyMode::Autonomous));
        assert_eq!(loaded.updated_at, "2025-06-01T00:00:00Z");
    }
}
