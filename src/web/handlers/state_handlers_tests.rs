//! Generated tests for `state_handlers.rs`.
//!
//! All endpoints here are local (no upstream proxy), so we assert real success
//! behaviour. `switch_theme` writes a KV file; we redirect `XDG_STATE_HOME` to a
//! temp dir so the real user state is never touched. The pure theme-resolution
//! helpers are exercised directly.

use super::*;
use crate::web::test_support::{send_json, test_router, test_server_state};
use crate::web::types::WebThemePair;
use axum::http::StatusCode;
use serde_json::json;

/// Serialises `XDG_STATE_HOME` mutation across the theme-switch tests so each
/// gets its own isolated KV directory (they both write `opencode/kv.json`).
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as STATE_LOCK;

/// A fully-specified WebThemeColors JSON object (all fields snake_case).
fn theme_colors_json() -> serde_json::Value {
    json!({
        "primary": "#111111",
        "secondary": "#222222",
        "accent": "#333333",
        "background": "#444444",
        "background_panel": "#555555",
        "background_element": "#666666",
        "text": "#777777",
        "text_muted": "#888888",
        "border": "#999999",
        "border_active": "#aaaaaa",
        "border_subtle": "#bbbbbb",
        "error": "#cccccc",
        "warning": "#dddddd",
        "success": "#eeeeee",
        "info": "#ffffff"
    })
}

fn theme_pair() -> WebThemePair {
    serde_json::from_value(json!({ "dark": theme_colors_json(), "light": theme_colors_json() }))
        .expect("valid WebThemePair")
}

// ── public_bootstrap ────────────────────────────────────────────────

#[tokio::test]
async fn public_bootstrap_ok_theme_null_when_unset() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/public/bootstrap", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["theme"].is_null());
    assert!(v["instance_name"].is_null());
}

// ── get_state ───────────────────────────────────────────────────────

#[tokio::test]
async fn get_state_ok_with_backend() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/state", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["backend"], "opencode");
    assert!(v["projects"].is_array());
}

// ── get_session_stats ───────────────────────────────────────────────

#[tokio::test]
async fn get_session_stats_defaults_when_missing() {
    let state = test_server_state();
    let (status, body) =
        send_json(test_router(state), "GET", "/api/session/nope/stats", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["cost"], 0.0);
    assert_eq!(v["input_tokens"], 0);
}

// ── get_theme ───────────────────────────────────────────────────────

#[tokio::test]
async fn get_theme_not_found_when_unset() {
    let state = test_server_state();
    let (status, _) = send_json(test_router(state), "GET", "/api/theme", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_theme_ok_after_set() {
    let state = test_server_state();
    state.web_state.set_theme(theme_pair()).await;
    let (status, body) = send_json(test_router(state), "GET", "/api/theme", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["dark"]["primary"], "#111111");
}

// ── list_themes ─────────────────────────────────────────────────────

#[tokio::test]
async fn list_themes_ok_non_empty() {
    let state = test_server_state();
    let (status, body) = send_json(test_router(state), "GET", "/api/themes", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = v.as_array().expect("array");
    assert!(!arr.is_empty());
    // Each entry has a name and both variants.
    assert!(arr[0]["name"].is_string());
    assert!(arr[0]["dark"].is_object());
    assert!(arr[0]["light"].is_object());
}

// ── switch_theme ────────────────────────────────────────────────────

#[tokio::test]
async fn switch_theme_writes_kv_and_returns_pair() {
    let state_home = tempfile::tempdir().unwrap();
    let state = test_server_state();

    let _guard = STATE_LOCK.lock().unwrap();
    let old = std::env::var_os("XDG_STATE_HOME");
    std::env::set_var("XDG_STATE_HOME", state_home.path());
    let (status, body) = send_json(
        test_router(state),
        "POST",
        "/api/theme/switch",
        Some(json!({ "name": "tokyonight" })),
    )
    .await;
    match old {
        Some(v) => std::env::set_var("XDG_STATE_HOME", v),
        None => std::env::remove_var("XDG_STATE_HOME"),
    }

    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["dark"].is_object());
    assert!(v["light"].is_object());

    let kv = state_home.path().join("opencode/kv.json");
    assert!(kv.exists(), "kv.json should have been written");
    let content = std::fs::read_to_string(&kv).unwrap();
    let kv_json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(kv_json["theme"], "tokyonight");
}

#[tokio::test]
async fn switch_theme_merges_existing_kv() {
    let state_home = tempfile::tempdir().unwrap();
    // Pre-seed an existing kv.json with an unrelated key.
    let dir = state_home.path().join("opencode");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kv.json"), r#"{"other":"keep"}"#).unwrap();

    let state = test_server_state();

    let _guard = STATE_LOCK.lock().unwrap();
    let old = std::env::var_os("XDG_STATE_HOME");
    std::env::set_var("XDG_STATE_HOME", state_home.path());
    let (status, _) = send_json(
        test_router(state),
        "POST",
        "/api/theme/switch",
        Some(json!({ "name": "gruvbox" })),
    )
    .await;
    match old {
        Some(v) => std::env::set_var("XDG_STATE_HOME", v),
        None => std::env::remove_var("XDG_STATE_HOME"),
    }

    assert_eq!(status, StatusCode::OK);
    let content = std::fs::read_to_string(dir.join("kv.json")).unwrap();
    let kv_json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(kv_json["theme"], "gruvbox");
    assert_eq!(kv_json["other"], "keep");
}

// ── resolve_theme_color (pure) ──────────────────────────────────────

#[test]
fn resolve_color_hex_literal() {
    let defs = json!({});
    let defs = defs.as_object().unwrap();
    assert_eq!(
        resolve_theme_color(&json!("#abcdef"), defs, "dark"),
        Some("#abcdef".to_string())
    );
}

#[test]
fn resolve_color_def_reference() {
    let defs = json!({ "blue": "#0000ff" });
    let defs = defs.as_object().unwrap();
    assert_eq!(
        resolve_theme_color(&json!("blue"), defs, "dark"),
        Some("#0000ff".to_string())
    );
}

#[test]
fn resolve_color_missing_def_is_none() {
    let defs = json!({});
    let defs = defs.as_object().unwrap();
    assert_eq!(resolve_theme_color(&json!("missing"), defs, "dark"), None);
}

#[test]
fn resolve_color_object_mode_variant() {
    let defs = json!({});
    let defs = defs.as_object().unwrap();
    let value = json!({ "dark": "#111111", "light": "#222222" });
    assert_eq!(
        resolve_theme_color(&value, defs, "light"),
        Some("#222222".to_string())
    );
    assert_eq!(
        resolve_theme_color(&value, defs, "dark"),
        Some("#111111".to_string())
    );
}

#[test]
fn resolve_color_object_falls_back_to_dark() {
    let defs = json!({});
    let defs = defs.as_object().unwrap();
    // No "light" key → falls back to "dark".
    let value = json!({ "dark": "#333333" });
    assert_eq!(
        resolve_theme_color(&value, defs, "light"),
        Some("#333333".to_string())
    );
}

#[test]
fn resolve_color_object_nested_def_reference() {
    let defs = json!({ "blue": "#0000ff" });
    let defs = defs.as_object().unwrap();
    let value = json!({ "dark": "blue" });
    assert_eq!(
        resolve_theme_color(&value, defs, "dark"),
        Some("#0000ff".to_string())
    );
}

#[test]
fn resolve_color_empty_object_is_none() {
    let defs = json!({});
    let defs = defs.as_object().unwrap();
    assert_eq!(resolve_theme_color(&json!({}), defs, "dark"), None);
}

#[test]
fn resolve_color_number_is_none() {
    let defs = json!({});
    let defs = defs.as_object().unwrap();
    assert_eq!(resolve_theme_color(&json!(5), defs, "dark"), None);
}

// ── resolve_theme_preview (pure) ────────────────────────────────────

#[test]
fn resolve_preview_valid_resolves_primary() {
    let jsonv = json!({ "defs": { "p": "#111111" }, "theme": { "primary": "p" } });
    let colors = resolve_theme_preview(&jsonv, "dark").expect("ok");
    assert_eq!(colors.primary, "#111111");
    // Unspecified fields fall back to their defaults.
    assert_eq!(colors.secondary, "#5c9cf5");
}

#[test]
fn resolve_preview_missing_defs_is_err() {
    let jsonv = json!({ "theme": { "primary": "#fff" } });
    assert!(resolve_theme_preview(&jsonv, "dark").is_err());
}

#[test]
fn resolve_preview_missing_theme_is_err() {
    let jsonv = json!({ "defs": {} });
    assert!(resolve_theme_preview(&jsonv, "dark").is_err());
}

#[test]
fn resolve_preview_all_defaults_when_empty_theme() {
    let jsonv = json!({ "defs": {}, "theme": {} });
    let colors = resolve_theme_preview(&jsonv, "dark").expect("ok");
    assert_eq!(colors.primary, "#fab283");
    assert_eq!(colors.background, "#0a0a0a");
    assert_eq!(colors.info, "#56b6c2");
}
