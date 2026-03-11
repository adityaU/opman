//! State, theme, and theme-switching handlers.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::*;

// ── State endpoints (independent — no TUI dependency) ───────────────

pub async fn get_state(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let mut app_state = state.web_state.get_state().await;
    app_state.instance_name = state.instance_name.clone();
    Ok(Json(app_state))
}

pub async fn get_session_stats(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let stats = state
        .web_state
        .get_session_stats(&session_id)
        .await
        .unwrap_or_default();
    Ok(Json(stats).into_response())
}

pub async fn get_theme(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    match state.web_state.get_theme().await {
        Some(t) => Ok(Json(t)),
        None => Err(WebError::NotFound("Theme not set yet")),
    }
}

/// GET /api/themes — list all available themes with preview colors.
pub async fn list_themes(
    State(_state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    use include_dir::{include_dir, Dir};
    static OPENCODE_THEMES: Dir = include_dir!("$CARGO_MANIFEST_DIR/opencode-themes");

    let mut themes: Vec<ThemePreview> = Vec::new();

    for entry in OPENCODE_THEMES.files() {
        if let Some(name) = entry.path().file_stem() {
            let name_str = name.to_string_lossy().to_string();
            // Parse the theme JSON to extract preview colors
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(entry.contents()) {
                if let Ok(colors) = resolve_theme_preview(&json) {
                    themes.push(ThemePreview {
                        name: name_str,
                        colors,
                    });
                }
            }
        }
    }

    // Sort alphabetically
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(themes))
}

/// POST /api/theme/switch — switch the active theme by writing to the KV store.
pub async fn switch_theme(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SwitchThemeRequest>,
) -> WebResult<impl IntoResponse> {
    use std::path::PathBuf;

    // Write the theme name to the KV store
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".local/state")
        });
    let kv_path = state_dir.join("opencode/kv.json");

    // Read existing KV or start fresh
    let mut kv: serde_json::Value = if kv_path.exists() {
        let content = tokio::fs::read_to_string(&kv_path).await
            .map_err(|e| WebError::Internal(format!("Failed to read KV store: {e}")))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Update theme name
    kv["theme"] = serde_json::Value::String(req.name.clone());

    // Ensure parent dir exists
    if let Some(parent) = kv_path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| WebError::Internal(format!("Failed to create KV dir: {e}")))?;
    }

    tokio::fs::write(&kv_path, serde_json::to_string_pretty(&kv).unwrap_or_default()).await
        .map_err(|e| WebError::Internal(format!("Failed to write KV store: {e}")))?;

    // Reload theme and broadcast to SSE clients
    let new_theme = crate::theme::load_theme();
    let web_colors = WebThemeColors::from_theme(&new_theme);
    state.web_state.set_theme(web_colors.clone()).await;

    // Broadcast theme change to all connected SSE clients
    let _ = state.event_tx.send(WebEvent::ThemeChanged(web_colors.clone()));

    Ok(Json(web_colors))
}

/// Resolve a theme JSON into preview colors (minimal version of theme::parse_theme).
pub(super) fn resolve_theme_preview(json: &serde_json::Value) -> Result<WebThemeColors, ()> {
    let defs = json.get("defs").and_then(|v| v.as_object()).ok_or(())?;
    let theme = json.get("theme").and_then(|v| v.as_object()).ok_or(())?;

    let resolve = |field: &str, fallback: &str| -> String {
        theme
            .get(field)
            .and_then(|v| resolve_theme_color(v, defs, "dark"))
            .unwrap_or_else(|| fallback.to_string())
    };

    Ok(WebThemeColors {
        primary: resolve("primary", "#fab283"),
        secondary: resolve("secondary", "#5c9cf5"),
        accent: resolve("accent", "#9d7cd8"),
        background: resolve("background", "#0a0a0a"),
        background_panel: resolve("backgroundPanel", "#141414"),
        background_element: resolve("backgroundElement", "#1e1e1e"),
        text: resolve("text", "#eeeeee"),
        text_muted: resolve("textMuted", "#808080"),
        border: resolve("border", "#484848"),
        border_active: resolve("borderActive", "#606060"),
        border_subtle: resolve("borderSubtle", "#3c3c3c"),
        error: resolve("error", "#e06c75"),
        warning: resolve("warning", "#f5a742"),
        success: resolve("success", "#7fd88f"),
        info: resolve("info", "#56b6c2"),
    })
}

/// Resolve a single color value through the defs reference chain.
pub(super) fn resolve_theme_color(
    value: &serde_json::Value,
    defs: &serde_json::Map<String, serde_json::Value>,
    mode: &str,
) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with('#') {
                Some(s.clone())
            } else {
                defs.get(s.as_str())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
        }
        serde_json::Value::Object(map) => {
            let variant = map.get(mode).or_else(|| map.get("dark"));
            variant.and_then(|v| resolve_theme_color(v, defs, mode))
        }
        _ => None,
    }
}
