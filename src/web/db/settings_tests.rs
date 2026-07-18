//! Generated coverage tests for `db/settings.rs`: all autonomy-mode conversions
//! and the upsert (INSERT then ON CONFLICT UPDATE) path.
use super::*;

#[test]
fn autonomy_mode_conversions() {
    assert_eq!(autonomy_mode_str(&AutonomyMode::Observe), "observe");
    assert_eq!(autonomy_mode_str(&AutonomyMode::Nudge), "nudge");
    assert_eq!(autonomy_mode_str(&AutonomyMode::Continue), "continue");
    assert_eq!(autonomy_mode_str(&AutonomyMode::Autonomous), "autonomous");

    assert!(matches!(parse_autonomy_mode("nudge"), AutonomyMode::Nudge));
    assert!(matches!(parse_autonomy_mode("continue"), AutonomyMode::Continue));
    assert!(matches!(parse_autonomy_mode("autonomous"), AutonomyMode::Autonomous));
    assert!(matches!(parse_autonomy_mode("observe"), AutonomyMode::Observe));
    assert!(matches!(parse_autonomy_mode("???"), AutonomyMode::Observe));
}

#[test]
fn save_then_overwrite_uses_conflict_update() {
    let db = Db::open_memory().unwrap();
    db.save_autonomy_settings(&AutonomySettings {
        mode: AutonomyMode::Nudge,
        updated_at: "2025-01-01T00:00:00Z".into(),
    });
    assert!(matches!(db.load_autonomy_settings().mode, AutonomyMode::Nudge));

    // Second save on the same id=1 row exercises the ON CONFLICT UPDATE branch.
    db.save_autonomy_settings(&AutonomySettings {
        mode: AutonomyMode::Continue,
        updated_at: "2025-02-01T00:00:00Z".into(),
    });
    let loaded = db.load_autonomy_settings();
    assert!(matches!(loaded.mode, AutonomyMode::Continue));
    assert_eq!(loaded.updated_at, "2025-02-01T00:00:00Z");
}

#[test]
fn load_defaults_to_observe_when_unset() {
    let db = Db::open_memory().unwrap();
    assert!(matches!(db.load_autonomy_settings().mode, AutonomyMode::Observe));
}
