use super::*;
use std::ffi::OsString;
use std::fs;
use std::sync::Mutex;
use tempfile::TempDir;

// Serialize all tests that mutate process environment variables. Survives
// poisoning so one failing test does not cascade into the rest.
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as ENV_LOCK;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// RAII guard that sets env vars and restores their previous values on drop
/// (even if a test panics).
struct EnvGuard {
    saved: Vec<(String, Option<OsString>)>,
}

impl EnvGuard {
    fn new(pairs: &[(&str, Option<&std::path::Path>)]) -> Self {
        let mut saved = Vec::new();
        for (k, v) in pairs {
            saved.push(((*k).to_string(), std::env::var_os(k)));
            match v {
                Some(p) => std::env::set_var(k, p),
                None => std::env::remove_var(k),
            }
        }
        EnvGuard { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, old) in &self.saved {
            match old {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

fn uniq(prefix: &str) -> String {
    format!("{}_{}", prefix, rand::random::<u64>())
}

/// A minimal but valid opencode theme JSON, with dark/light variants for primary.
fn theme_json(primary_dark: &str, primary_light: &str) -> String {
    format!(
        r#"{{"defs":{{"pd":"{}","pl":"{}"}},"theme":{{"primary":{{"dark":"pd","light":"pl"}}}}}}"#,
        primary_dark, primary_light
    )
}

// ---------------------------------------------------------------------------
// read_theme_from_kv
// ---------------------------------------------------------------------------

#[test]
fn kv_present_returns_theme_and_mode() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    let oc = tmp.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("kv.json"), r#"{"theme":"mytheme","theme_mode":"light"}"#).unwrap();
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", Some(tmp.path()))]);
    assert_eq!(
        read_theme_from_kv(),
        Some(("mytheme".to_string(), "light".to_string()))
    );
}

#[test]
fn kv_missing_file_returns_none() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", Some(tmp.path()))]);
    assert_eq!(read_theme_from_kv(), None);
}

#[test]
fn kv_invalid_json_returns_none() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    let oc = tmp.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("kv.json"), "{not valid json").unwrap();
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", Some(tmp.path()))]);
    assert_eq!(read_theme_from_kv(), None);
}

#[test]
fn kv_missing_theme_field_returns_none() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    let oc = tmp.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("kv.json"), r#"{"theme_mode":"dark"}"#).unwrap();
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", Some(tmp.path()))]);
    assert_eq!(read_theme_from_kv(), None);
}

#[test]
fn kv_theme_without_mode_defaults_dark() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    let oc = tmp.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("kv.json"), r#"{"theme":"solo"}"#).unwrap();
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", Some(tmp.path()))]);
    assert_eq!(
        read_theme_from_kv(),
        Some(("solo".to_string(), "dark".to_string()))
    );
}

#[test]
fn kv_home_fallback_when_xdg_state_unset() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    // No kv.json under HOME/.local/state/opencode -> None, but exercises the
    // home_dir() fallback branch.
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", None), ("HOME", Some(tmp.path()))]);
    assert_eq!(read_theme_from_kv(), None);
}

// ---------------------------------------------------------------------------
// read_active_theme_name
// ---------------------------------------------------------------------------

#[test]
fn active_theme_from_kv_short_circuits() {
    let _l = env_lock();
    let tmp = TempDir::new().unwrap();
    let oc = tmp.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("kv.json"), r#"{"theme":"kvtheme","theme_mode":"light"}"#).unwrap();
    let _e = EnvGuard::new(&[("XDG_STATE_HOME", Some(tmp.path()))]);
    assert_eq!(
        read_active_theme_name().unwrap(),
        ("kvtheme".to_string(), "light".to_string())
    );
}

#[test]
fn active_theme_from_config_top_level() {
    let _l = env_lock();
    let state = TempDir::new().unwrap(); // empty -> kv miss
    let cfg = TempDir::new().unwrap();
    let oc = cfg.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(
        oc.join("opencode.json"),
        "{\n  // pick a theme\n  \"theme\": \"toptheme\"\n}",
    )
    .unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(state.path())),
        ("XDG_CONFIG_HOME", Some(cfg.path())),
    ]);
    assert_eq!(
        read_active_theme_name().unwrap(),
        ("toptheme".to_string(), "dark".to_string())
    );
}

#[test]
fn active_theme_from_config_nested_pointer() {
    let _l = env_lock();
    let state = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();
    let oc = cfg.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(
        oc.join("opencode.json"),
        r#"{"sync":{"data":{"config":{"theme":"nested"}}}}"#,
    )
    .unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(state.path())),
        ("XDG_CONFIG_HOME", Some(cfg.path())),
    ]);
    assert_eq!(
        read_active_theme_name().unwrap(),
        ("nested".to_string(), "dark".to_string())
    );
}

#[test]
fn active_theme_no_config_defaults_opencode() {
    let _l = env_lock();
    let state = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap(); // no opencode dir -> no config candidate exists
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(state.path())),
        ("XDG_CONFIG_HOME", Some(cfg.path())),
    ]);
    assert_eq!(
        read_active_theme_name().unwrap(),
        ("opencode".to_string(), "dark".to_string())
    );
}

#[test]
fn active_theme_config_invalid_json_errors() {
    let _l = env_lock();
    let state = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();
    let oc = cfg.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("opencode.json"), "{ this is not json").unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(state.path())),
        ("XDG_CONFIG_HOME", Some(cfg.path())),
    ]);
    assert!(read_active_theme_name().is_err());
}

#[test]
fn active_theme_config_missing_theme_field_defaults() {
    let _l = env_lock();
    let state = TempDir::new().unwrap();
    let cfg = TempDir::new().unwrap();
    let oc = cfg.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(oc.join("opencode.json"), r#"{"other":true}"#).unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(state.path())),
        ("XDG_CONFIG_HOME", Some(cfg.path())),
    ]);
    // theme lookups all miss -> unwrap_or("opencode")
    assert_eq!(
        read_active_theme_name().unwrap(),
        ("opencode".to_string(), "dark".to_string())
    );
}

// ---------------------------------------------------------------------------
// load_theme_json
// ---------------------------------------------------------------------------

#[test]
fn load_theme_json_found_in_config() {
    let _l = env_lock();
    let cfg = TempDir::new().unwrap();
    let name = uniq("found");
    let themes = cfg.path().join("opencode/themes");
    fs::create_dir_all(&themes).unwrap();
    fs::write(
        themes.join(format!("{}.json", name)),
        theme_json("#111111", "#222222"),
    )
    .unwrap();
    let _e = EnvGuard::new(&[("XDG_CONFIG_HOME", Some(cfg.path()))]);
    let val = load_theme_json(&name).unwrap();
    assert!(val.get("theme").is_some());
    assert!(val.get("defs").is_some());
}

#[test]
fn load_theme_json_invalid_json_errors() {
    let _l = env_lock();
    let cfg = TempDir::new().unwrap();
    let name = uniq("bad");
    let themes = cfg.path().join("opencode/themes");
    fs::create_dir_all(&themes).unwrap();
    fs::write(themes.join(format!("{}.json", name)), "{not json").unwrap();
    let _e = EnvGuard::new(&[("XDG_CONFIG_HOME", Some(cfg.path()))]);
    assert!(load_theme_json(&name).is_err());
}

#[test]
fn load_theme_json_not_found_bails() {
    let _l = env_lock();
    let cfg = TempDir::new().unwrap();
    let name = uniq("nowhere");
    let _e = EnvGuard::new(&[("XDG_CONFIG_HOME", Some(cfg.path()))]);
    let err = load_theme_json(&name).unwrap_err();
    assert!(err.to_string().contains(&name));
    assert!(err.to_string().contains("not found"));
}

// ---------------------------------------------------------------------------
// deploy_embedded_themes
// ---------------------------------------------------------------------------

#[test]
fn deploy_embedded_themes_writes_files() {
    let _l = env_lock();
    let cfg = TempDir::new().unwrap();
    let _e = EnvGuard::new(&[("XDG_CONFIG_HOME", Some(cfg.path()))]);
    deploy_embedded_themes().unwrap();
    let themes = cfg.path().join("opencode/themes");
    assert!(themes.is_dir());
    let count = fs::read_dir(&themes).unwrap().count();
    assert!(count > 0, "expected embedded themes to be written");
    // A known bundled theme should be present and non-empty.
    let aura = themes.join("aura.json");
    assert!(aura.exists());
    assert!(!fs::read_to_string(&aura).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// load_theme / load_theme_with_mode (end to end)
// ---------------------------------------------------------------------------

#[test]
fn load_theme_end_to_end_ok() {
    let _l = env_lock();
    let dir = TempDir::new().unwrap();
    let name = uniq("live");
    // kv points at our theme
    let oc = dir.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(
        oc.join("kv.json"),
        format!(r#"{{"theme":"{}","theme_mode":"dark"}}"#, name),
    )
    .unwrap();
    // theme file in config themes dir
    let themes = dir.path().join("opencode/themes");
    fs::create_dir_all(&themes).unwrap();
    fs::write(
        themes.join(format!("{}.json", name)),
        theme_json("#010203", "#0a0b0c"),
    )
    .unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(dir.path())),
        ("XDG_CONFIG_HOME", Some(dir.path())),
    ]);
    let colors = load_theme();
    assert_eq!(colors.primary, ratatui::style::Color::Rgb(0x01, 0x02, 0x03));
}

#[test]
fn load_theme_falls_back_to_default_on_error() {
    let _l = env_lock();
    let dir = TempDir::new().unwrap();
    let name = uniq("missingfile");
    let oc = dir.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(
        oc.join("kv.json"),
        format!(r#"{{"theme":"{}","theme_mode":"dark"}}"#, name),
    )
    .unwrap();
    // No theme file exists anywhere -> load fails -> default.
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(dir.path())),
        ("XDG_CONFIG_HOME", Some(dir.path())),
    ]);
    let colors = load_theme();
    let def = ThemeColors::default();
    assert_eq!(colors.primary, def.primary);
    assert_eq!(colors.background, def.background);
}

#[test]
fn load_theme_with_mode_ok_light() {
    let _l = env_lock();
    let dir = TempDir::new().unwrap();
    let name = uniq("modelive");
    let oc = dir.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(
        oc.join("kv.json"),
        format!(r#"{{"theme":"{}","theme_mode":"dark"}}"#, name),
    )
    .unwrap();
    let themes = dir.path().join("opencode/themes");
    fs::create_dir_all(&themes).unwrap();
    fs::write(
        themes.join(format!("{}.json", name)),
        theme_json("#010203", "#aabbcc"),
    )
    .unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(dir.path())),
        ("XDG_CONFIG_HOME", Some(dir.path())),
    ]);
    // Explicit light mode should resolve the light variant.
    let colors = load_theme_with_mode("light");
    assert_eq!(colors.primary, ratatui::style::Color::Rgb(0xaa, 0xbb, 0xcc));
}

#[test]
fn load_theme_with_mode_falls_back_to_default() {
    let _l = env_lock();
    let dir = TempDir::new().unwrap();
    let name = uniq("modemissing");
    let oc = dir.path().join("opencode");
    fs::create_dir_all(&oc).unwrap();
    fs::write(
        oc.join("kv.json"),
        format!(r#"{{"theme":"{}","theme_mode":"dark"}}"#, name),
    )
    .unwrap();
    let _e = EnvGuard::new(&[
        ("XDG_STATE_HOME", Some(dir.path())),
        ("XDG_CONFIG_HOME", Some(dir.path())),
    ]);
    let colors = load_theme_with_mode("dark");
    assert_eq!(colors.primary, ThemeColors::default().primary);
}
