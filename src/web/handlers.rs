//! REST API route handlers.
//!
//! Authentication is enforced via the `AuthUser` extractor — handlers that
//! include it in their signature automatically reject unauthenticated requests.
//!
//! State queries use the independent `WebStateHandle` (no TUI dependency).
//! Terminal I/O goes directly to the `WebPtyManager` (independent web PTYs).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Serialize;

use super::auth::{self, AuthUser};
use super::error::{WebError, WebResult};
use super::types::*;

// ── Auth endpoints ──────────────────────────────────────────────────

pub async fn login(
    State(state): State<ServerState>,
    Json(req): Json<LoginRequest>,
) -> WebResult<impl IntoResponse> {
    if state.username.is_empty() {
        let token = auth::create_jwt("anonymous", &state.jwt_secret)?;
        return Ok(Json(LoginResponse { token }));
    }
    if req.username == state.username && req.password == state.password {
        let token = auth::create_jwt(&req.username, &state.jwt_secret)?;
        Ok(Json(LoginResponse { token }))
    } else {
        Err(WebError::Unauthorized)
    }
}

pub async fn verify(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let _ = state;
    Ok(StatusCode::OK)
}

// ── State endpoints (independent — no TUI dependency) ───────────────

pub async fn get_state(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let app_state = state.web_state.get_state().await;
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
        let content = std::fs::read_to_string(&kv_path)
            .map_err(|e| WebError::Internal(format!("Failed to read KV store: {e}")))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Update theme name
    kv["theme"] = serde_json::Value::String(req.name.clone());

    // Ensure parent dir exists
    if let Some(parent) = kv_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WebError::Internal(format!("Failed to create KV dir: {e}")))?;
    }

    std::fs::write(&kv_path, serde_json::to_string_pretty(&kv).unwrap_or_default())
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
fn resolve_theme_preview(json: &serde_json::Value) -> Result<WebThemeColors, ()> {
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
fn resolve_theme_color(
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

// ── Action endpoints (independent — no TUI dependency) ──────────────

pub async fn switch_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SwitchProjectRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.switch_project(req.index).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project index".into()))
    }
}

pub async fn select_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SelectSessionRequest>,
) -> WebResult<impl IntoResponse> {
    if state
        .web_state
        .select_session(req.project_idx, req.session_id)
        .await
    {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project or session".into()))
    }
}

pub async fn new_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<NewSessionRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.new_session(req.project_idx).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Invalid project index".into()))
    }
}

pub async fn toggle_panel(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<TogglePanelRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.toggle_panel(&req.panel).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Unknown panel name".into()))
    }
}

pub async fn focus_panel(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FocusPanelRequest>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.focus_panel(&req.panel).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("Unknown panel name".into()))
    }
}

// ── Web PTY endpoints (independent from TUI) ────────────────────────

#[derive(Serialize)]
struct SpawnResponse {
    id: String,
    ok: bool,
}

/// Spawn a new web-owned PTY (shell, neovim, gitui, or opencode).
pub async fn spawn_pty(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<SpawnPtyRequest>,
) -> WebResult<impl IntoResponse> {
    let rows = req.rows.unwrap_or(24);
    let cols = req.cols.unwrap_or(80);

    // Get the working directory from the active project
    let working_dir = state
        .web_state
        .get_working_dir()
        .await
        .ok_or(WebError::BadRequest("No active project".into()))?;

    let result = match req.kind.as_str() {
        "shell" => {
            state
                .pty_mgr
                .spawn_shell(req.id.clone(), rows, cols, working_dir)
                .await
        }
        "neovim" => {
            state
                .pty_mgr
                .spawn_neovim(req.id.clone(), rows, cols, working_dir)
                .await
        }
        "git" => {
            state
                .pty_mgr
                .spawn_gitui(req.id.clone(), rows, cols, working_dir)
                .await
        }
        "opencode" => {
            // Get the active session ID (if any) to attach to
            let session_id = req.session_id.clone().or_else(|| {
                // Try to get from web state synchronously — but we're in async context
                None
            });
            // We'll resolve session_id from web state if not provided
            let session_id = match session_id {
                Some(sid) => Some(sid),
                None => state.web_state.active_session_id().await,
            };
            state
                .pty_mgr
                .spawn_opencode(req.id.clone(), rows, cols, working_dir, session_id)
                .await
        }
        _ => {
            return Err(WebError::BadRequest(format!(
                "Unknown PTY kind: {}",
                req.kind
            )))
        }
    };

    match result {
        Ok(_) => Ok(Json(SpawnResponse {
            id: req.id,
            ok: true,
        })),
        Err(e) => Err(WebError::Internal(format!("Failed to spawn PTY: {}", e))),
    }
}

/// Write bytes to a web-owned PTY.
pub async fn pty_write(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyWriteRequest>,
) -> WebResult<impl IntoResponse> {
    let data = BASE64
        .decode(&req.data)
        .map_err(|e| WebError::BadRequest(format!("Invalid base64: {e}")))?;
    let ok = state.pty_mgr.write(&req.id, data).await;
    if ok {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

/// Resize a web-owned PTY.
pub async fn pty_resize(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyResizeRequest>,
) -> WebResult<impl IntoResponse> {
    let ok = state.pty_mgr.resize(&req.id, req.rows, req.cols).await;
    if ok {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

/// Kill a web-owned PTY.
pub async fn pty_kill(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<PtyKillRequest>,
) -> WebResult<impl IntoResponse> {
    let ok = state.pty_mgr.kill(&req.id).await;
    if ok {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::BadRequest("PTY not found".into()))
    }
}

/// List active web PTY IDs.
pub async fn pty_list(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let ids = state.pty_mgr.list().await;
    Ok(Json(ids))
}

// ── Proxy endpoints (forward to opencode server) ────────────────────

use crate::api::ApiClient;
use crate::app::base_url;

/// Helper: resolve project directory from web state.
async fn resolve_project_dir(state: &ServerState) -> WebResult<String> {
    state
        .web_state
        .get_working_dir()
        .await
        .map(|p| p.to_string_lossy().to_string())
        .ok_or(WebError::BadRequest("No active project".into()))
}

/// GET /api/session/:id/messages — fetch all messages for a session.
///
/// Returns `{ "messages": [...] }` sorted by creation time.
pub async fn get_session_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    // Normalise the response into a flat Vec — upstream may return an array
    // or an object keyed by message ID.
    let mut all_messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.clone()
    } else if let Some(obj) = body.as_object() {
        obj.values().cloned().collect()
    } else {
        vec![]
    };

    // Sort by info.time.created to ensure chronological order.
    all_messages.sort_by(|a, b| {
        let time_a = a.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0);
        let time_b = b.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0);
        time_a.cmp(&time_b)
    });

    Ok(Json(serde_json::json!({
        "messages": all_messages,
    })))
}

/// POST /api/session/:id/message — send a message to a session.
pub async fn send_message(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/session/{}/message", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&req)
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        return Err(WebError::Internal(format!("Upstream {}: {:?}", status, body)));
    }
    Ok(Json(body))
}

/// POST /api/session/:id/abort — abort a running session.
pub async fn abort_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    client
        .abort_session(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// POST /api/session/:id/command — execute a slash command.
pub async fn execute_command(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<ExecuteCommandRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    let result = client
        .execute_session_command(
            &base,
            &dir,
            &session_id,
            &req.command,
            &req.arguments,
            req.model.as_deref(),
        )
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(result))
}

/// GET /api/providers — fetch available providers and models.
pub async fn get_providers(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    let providers = client
        .fetch_providers(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(providers))
}

/// GET /api/commands — list available slash commands.
pub async fn get_commands(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    let cmds = client
        .list_commands(&base, &dir)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(cmds))
}

/// POST /api/permission/:id/reply — reply to a permission request.
pub async fn reply_permission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<PermissionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    client
        .reply_permission(&base, &dir, &request_id, &req.reply)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// POST /api/question/:id/reply — reply to a question.
pub async fn reply_question(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(req): Json<QuestionReplyRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    client
        .reply_question(&base, &dir, &request_id, &req.answers)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// GET /api/session/:id/todos — fetch todos for a session.
pub async fn get_session_todos(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::new();
    let todos = client
        .fetch_todos(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(todos))
}
