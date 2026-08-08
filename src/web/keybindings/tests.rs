use super::types::{Binding, Mode};
use super::*;
use std::sync::{Mutex, OnceLock};

/// `OPMAN_KEYBINDINGS_CONFIG` is process-wide, so the tests that set it must not
/// interleave.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct TempConfig {
    dir: tempfile::TempDir,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl TempConfig {
    fn new() -> Self {
        let guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var(
            "OPMAN_KEYBINDINGS_CONFIG",
            dir.path().join("keybindings.json"),
        );
        Self { dir, _guard: guard }
    }

    fn write(&self, contents: &str) {
        std::fs::write(self.dir.path().join("keybindings.json"), contents).expect("write");
    }
}

impl Drop for TempConfig {
    fn drop(&mut self) {
        std::env::remove_var("OPMAN_KEYBINDINGS_CONFIG");
    }
}

#[test]
fn config_path_prefers_the_env_override() {
    let temp = TempConfig::new();
    let expected = temp.dir.path().join("keybindings.json");
    assert_eq!(config_path(), Some(expected));
}

#[test]
fn missing_file_yields_defaults_without_a_diagnostic() {
    let _temp = TempConfig::new();
    let loaded = load();
    assert_eq!(loaded.config, KeybindingsConfig::default());
    assert!(loaded.diagnostics.is_empty());
}

#[test]
fn empty_object_is_valid_and_every_field_defaults() {
    let temp = TempConfig::new();
    temp.write("{}");
    let loaded = load();
    assert_eq!(loaded.config, KeybindingsConfig::default());
    assert!(loaded.diagnostics.is_empty());
}

#[test]
fn malformed_json_degrades_to_defaults_and_reports_a_position() {
    let temp = TempConfig::new();
    temp.write("{\n  \"mode\": \"vim\",\n  oops\n}");
    let loaded = load();

    assert_eq!(loaded.config, KeybindingsConfig::default());
    let diagnostic = loaded.diagnostics.first().expect("a diagnostic");
    assert_eq!(diagnostic.line, Some(3));
    assert!(diagnostic.column.is_some());
}

#[test]
fn bindings_round_trip_through_save_and_load() {
    let _temp = TempConfig::new();
    let config = KeybindingsConfig {
        mode: Mode::Vim,
        bindings: vec![
            Binding {
                key: "ctrl+k ctrl+w".to_string(),
                command: "session.close".to_string(),
                when: Some("sessionActive".to_string()),
                mode: None,
                platform: Some("mac".to_string()),
                target: None,
                browser: None,
                group: None,
                label: None,
            },
            Binding {
                key: "ctrl+shift+p".to_string(),
                command: "-palette.commands".to_string(),
                when: None,
                mode: None,
                platform: None,
                target: None,
                browser: None,
                group: None,
                label: None,
            },
        ],
        ..KeybindingsConfig::default()
    };

    save(&config).expect("save");
    let loaded = load();

    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.config, config);
}

#[test]
fn save_creates_the_config_directory() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir
        .path()
        .join("deeply")
        .join("nested")
        .join("keybindings.json");
    std::env::set_var("OPMAN_KEYBINDINGS_CONFIG", &nested);

    let saved = save(&KeybindingsConfig::default());
    std::env::remove_var("OPMAN_KEYBINDINGS_CONFIG");

    assert_eq!(saved.ok(), Some(nested.clone()));
    assert!(nested.exists());
}

#[test]
fn save_leaves_no_temporary_file_behind() {
    let temp = TempConfig::new();
    save(&KeybindingsConfig::default()).expect("save");
    assert!(!temp.dir.path().join("keybindings.json.tmp").exists());
}

#[test]
fn unknown_fields_are_ignored_rather_than_rejected() {
    let temp = TempConfig::new();
    temp.write("{\"mode\":\"vim\",\"somethingNew\":42}");
    let loaded = load();

    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.config.mode, Mode::Vim);
}
