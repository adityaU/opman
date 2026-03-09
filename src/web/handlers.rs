//! REST API route handlers.
//!
//! Authentication is enforced via the `AuthUser` extractor — handlers that
//! include it in their signature automatically reject unauthenticated requests.
//!
//! State queries use the independent `WebStateHandle` (no TUI dependency).
//! Terminal I/O goes directly to the `WebPtyManager` (independent web PTYs).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Serialize;
use std::path::PathBuf;

use super::auth::{self, AuthUser};
use super::error::{WebError, WebResult};
use super::types::*;

/// Constant-time byte comparison to prevent timing side-channel attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn resolve_editor_nvim_socket(
    state: &ServerState,
    session_id: &str,
) -> WebResult<PathBuf> {
    let project_idx = state.web_state.active_project_index().await;
    let registry = state.nvim_registry.read().await;
    registry
        .get(&(project_idx, session_id.to_string()))
        .cloned()
        .ok_or_else(|| WebError::BadRequest("No Neovim/LSP backend active for this session. Open a Neovim session first.".into()))
}

async fn resolve_editor_buffer(
    state: &ServerState,
    session_id: &str,
    path: &str,
) -> WebResult<(PathBuf, String, i64)> {
    let socket = resolve_editor_nvim_socket(state, session_id).await?;
    let project_dir = state
        .web_state
        .get_working_dir()
        .await
        .ok_or_else(|| WebError::BadRequest("No active project directory".into()))?;
    let resolved = if std::path::Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        project_dir.join(path)
    };
    let resolved_str = resolved.to_string_lossy().to_string();
    let buf = crate::nvim_rpc::nvim_find_or_load_buffer(&socket, &resolved_str)
        .map_err(|e| WebError::Internal(format!("Failed to load editor buffer: {e}")))?;
    Ok((socket, resolved_str, buf))
}

// ── Health check (unauthenticated) ──────────────────────────────────

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── Auth endpoints ──────────────────────────────────────────────────

pub async fn login(
    State(state): State<ServerState>,
    Json(req): Json<LoginRequest>,
) -> WebResult<impl IntoResponse> {
    if state.username.is_empty() {
        let token = auth::create_jwt("anonymous", &state.jwt_secret)?;
        return Ok(Json(LoginResponse { token }));
    }
    if req.username == state.username && constant_time_eq(req.password.as_bytes(), state.password.as_bytes()) {
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
    // Resolve the project directory for the opencode server header.
    let dir = state
        .web_state
        .get_project_working_dir(req.project_idx)
        .await
        .map(|p| p.to_string_lossy().to_string())
        .ok_or(WebError::BadRequest("Invalid project index".into()))?;

    // Create the session synchronously via the opencode server API.
    let base = base_url().to_string();
    let resp = state
        .http_client
        .post(format!("{}/session", base))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| WebError::Internal(format!("Parse error: {e}")))?;

    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }

    // Parse session info from the response.
    let session_info: crate::app::SessionInfo = serde_json::from_value(body.clone())
        .map_err(|e| WebError::Internal(format!("Failed to parse session info: {e}")))?;

    let session_id = session_info.id.clone();

    // Add the new session to web_state and set it as active.
    state
        .web_state
        .add_and_activate_session(req.project_idx, session_info)
        .await;

    Ok(Json(NewSessionResponse { session_id }))
}

/// POST /api/project/add — add a new project by directory path.
pub async fn add_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<AddProjectRequest>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.add_project(&req.path, req.name.as_deref()).await {
        Ok((index, name)) => Ok(Json(AddProjectResponse { index, name })),
        Err(msg) => Err(WebError::BadRequest(msg)),
    }
}

/// POST /api/project/remove — remove a project by index.
pub async fn remove_project(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<RemoveProjectRequest>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.remove_project(req.index).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(msg) => Err(WebError::BadRequest(msg)),
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
    let rows = req.rows.unwrap_or(24).clamp(1, 500);
    let cols = req.cols.unwrap_or(80).clamp(1, 500);

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
    let rows = req.rows.clamp(1, 500);
    let cols = req.cols.clamp(1, 500);
    let ok = state.pty_mgr.resize(&req.id, rows, cols).await;
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

/// Query parameters for paginated message fetching.
#[derive(serde::Deserialize)]
pub struct MessagePageQuery {
    /// Maximum number of messages to return. Omit or 0 for all.
    pub limit: Option<usize>,
    /// Only return messages created **before** this Unix-ms timestamp (exclusive).
    /// Used for "load older" pagination — pass the oldest timestamp from the
    /// previous page to fetch the preceding chunk.
    pub before: Option<u64>,
}

/// GET /api/session/:id/messages — fetch messages for a session.
///
/// Supports optional pagination via query parameters:
///   - `?limit=N`             — return only the N most recent messages
///   - `?before=TIMESTAMP`    — return messages before this Unix-ms timestamp
///   - `?limit=N&before=T`    — load N messages before timestamp T
///
/// Response: `{ "messages": [...], "has_more": bool, "total": usize }`
/// Messages are sorted by creation time (ascending — oldest first within the page).
pub async fn get_session_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Query(page): Query<MessagePageQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let resp = state.http_client
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

    let total = all_messages.len();

    // Apply pagination: filter by `before` timestamp, then take last `limit`.
    let limit = page.limit.unwrap_or(0);

    if limit > 0 || page.before.is_some() {
        // Filter by `before` — keep only messages with created < before
        if let Some(before_ts) = page.before {
            all_messages.retain(|m| {
                let ts = m.pointer("/info/time/created").and_then(|v| v.as_u64()).unwrap_or(0);
                ts < before_ts
            });
        }

        let filtered_count = all_messages.len();
        let effective_limit = if limit > 0 { limit } else { filtered_count };

        // Take only the last `limit` messages (most recent within the filtered set)
        let has_more = filtered_count > effective_limit;
        if has_more {
            all_messages = all_messages.split_off(filtered_count - effective_limit);
        }

        Ok(Json(serde_json::json!({
            "messages": all_messages,
            "has_more": has_more,
            "total": total,
        })))
    } else {
        // No pagination — return everything (backward compatible)
        Ok(Json(serde_json::json!({
            "messages": all_messages,
            "has_more": false,
            "total": total,
        })))
    }
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
    let resp = state.http_client
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
        tracing::error!(
            %session_id,
            upstream_status = %status,
            upstream_body = %body,
            "send_message: upstream rejected"
        );
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
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .abort_session(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

/// DELETE /api/session/:id — delete a session.
pub async fn delete_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let resp = state
        .http_client
        .delete(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(StatusCode::OK)
}

/// PATCH /api/session/:id — rename a session (update title).
pub async fn rename_session(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let resp = state
        .http_client
        .patch(format!("{}/session/{}", base, session_id))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "title": req.title }))
        .send()
        .await
        .map_err(|e| WebError::Internal(format!("Upstream error: {e}")))?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        return Err(WebError::Internal(format!(
            "Upstream {}: {:?}",
            status, body
        )));
    }
    Ok(Json(body))
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
    let client = ApiClient::with_client(state.http_client.clone());
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
    let client = ApiClient::with_client(state.http_client.clone());
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
    let client = ApiClient::with_client(state.http_client.clone());
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
    let client = ApiClient::with_client(state.http_client.clone());
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
    let client = ApiClient::with_client(state.http_client.clone());
    client
        .reply_question(&base, &dir, &request_id, &req.answers)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(StatusCode::OK)
}

// ── Git API endpoints (shell out to git CLI) ────────────────────────

/// GET /api/git/status — structured git status for the active project.
pub async fn git_status(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get branch name
    let branch_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // Get porcelain status
    let status_output = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git status: {e}")))?;
    let status_text = String::from_utf8_lossy(&status_output.stdout);

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in status_text.lines() {
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let path = line[3..].to_string();

        // Untracked
        if index_status == '?' {
            untracked.push(GitFileEntry {
                path,
                status: "?".to_string(),
            });
            continue;
        }

        // Staged changes (index column)
        if index_status != ' ' && index_status != '?' {
            staged.push(GitFileEntry {
                path: path.clone(),
                status: index_status.to_string(),
            });
        }

        // Unstaged changes (worktree column)
        if worktree_status != ' ' && worktree_status != '?' {
            unstaged.push(GitFileEntry {
                path,
                status: worktree_status.to_string(),
            });
        }
    }

    Ok(Json(GitStatusResponse {
        branch,
        staged,
        unstaged,
        untracked,
    }))
}

/// GET /api/git/diff?file=...&staged=... — get diff for a file or all files.
pub async fn git_diff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitDiffQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    let mut args = vec!["diff".to_string()];
    if query.staged {
        args.push("--cached".to_string());
    }
    if let Some(ref file) = query.file {
        args.push("--".to_string());
        args.push(file.clone());
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git diff: {e}")))?;

    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(Json(GitDiffResponse { diff }))
}

/// GET /api/git/log?limit=50 — recent commits.
pub async fn git_log(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitLogQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);
    let limit = query.limit.unwrap_or(50).min(500); // Cap at 500 commits

    // Use a delimiter that won't appear in normal commit data
    let format = "%H%x1f%h%x1f%an%x1f%aI%x1f%s";
    let output = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={}", limit),
            &format!("--format={}", format),
        ])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git log: {e}")))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<GitLogEntry> = text
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\x1f').collect();
            if parts.len() >= 5 {
                Some(GitLogEntry {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                    message: parts[4].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(Json(GitLogResponse { commits }))
}

/// POST /api/git/stage — stage files.
pub async fn git_stage(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitStageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    let mut args = vec!["add".to_string()];
    if req.files.is_empty() {
        args.push("-A".to_string()); // Stage all
    } else {
        args.push("--".to_string());
        args.extend(req.files);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git add: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!("git add failed: {stderr}")));
    }

    Ok(StatusCode::OK)
}

/// POST /api/git/unstage — unstage files.
pub async fn git_unstage(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitUnstageRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    let mut args = vec!["restore".to_string(), "--staged".to_string()];
    if req.files.is_empty() {
        args.push(".".to_string()); // Unstage all
    } else {
        args.push("--".to_string());
        args.extend(req.files);
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git restore: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!(
            "git restore --staged failed: {stderr}"
        )));
    }

    Ok(StatusCode::OK)
}

/// POST /api/git/commit — create a commit.
pub async fn git_commit(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitCommitRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    if req.message.trim().is_empty() {
        return Err(WebError::BadRequest("Commit message cannot be empty".into()));
    }

    let output = tokio::process::Command::new("git")
        .args(["commit", "-m", &req.message])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git commit: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!("git commit failed: {stderr}")));
    }

    // Get the hash of the commit we just made
    let hash_output = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit hash: {e}")))?;

    let hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();

    Ok(Json(GitCommitResponse {
        hash,
        message: req.message,
    }))
}

/// POST /api/git/discard — discard unstaged changes for files.
pub async fn git_discard(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitDiscardRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    if req.files.is_empty() {
        return Err(WebError::BadRequest(
            "Must specify files to discard".into(),
        ));
    }

    let mut args = vec!["checkout".to_string(), "--".to_string()];
    args.extend(req.files);

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git checkout: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WebError::Internal(format!(
            "git checkout failed: {stderr}"
        )));
    }

    Ok(StatusCode::OK)
}

/// GET /api/git/show?hash=... — show a commit's diff and metadata.
pub async fn git_show(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<GitShowQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get commit metadata
    let format = "%H%x1f%an%x1f%aI%x1f%B";
    let meta_output = tokio::process::Command::new("git")
        .args(["show", "--no-patch", &format!("--format={}", format), &query.hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git show: {e}")))?;

    if !meta_output.status.success() {
        let stderr = String::from_utf8_lossy(&meta_output.stderr);
        return Err(WebError::BadRequest(format!("git show failed: {stderr}")));
    }

    let meta_text = String::from_utf8_lossy(&meta_output.stdout);
    let meta_parts: Vec<&str> = meta_text.trim().splitn(4, '\x1f').collect();
    let (hash, author, date, message) = if meta_parts.len() >= 4 {
        (
            meta_parts[0].to_string(),
            meta_parts[1].to_string(),
            meta_parts[2].to_string(),
            meta_parts[3].trim().to_string(),
        )
    } else {
        (query.hash.clone(), String::new(), String::new(), String::new())
    };

    // Get diff
    let diff_output = tokio::process::Command::new("git")
        .args(["show", "--format=", "--patch", &query.hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit diff: {e}")))?;

    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    // Get changed files list
    let files_output = tokio::process::Command::new("git")
        .args(["show", "--format=", "--name-status", &query.hash])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to get commit files: {e}")))?;

    let files_text = String::from_utf8_lossy(&files_output.stdout);
    let files: Vec<GitShowFile> = files_text
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                Some(GitShowFile {
                    status: parts[0].to_string(),
                    path: parts[1].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(Json(GitShowResponse {
        hash,
        author,
        date,
        message,
        diff,
        files,
    }))
}

/// GET /api/git/branches — list all local and remote branches.
pub async fn git_branches(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get current branch
    let head_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git: {e}")))?;
    let current = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_string();

    // Get local branches
    let local_output = tokio::process::Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to list local branches: {e}")))?;
    let local: Vec<String> = String::from_utf8_lossy(&local_output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Get remote branches
    let remote_output = tokio::process::Command::new("git")
        .args(["branch", "-r", "--format=%(refname:short)"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to list remote branches: {e}")))?;
    let remote: Vec<String> = String::from_utf8_lossy(&remote_output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !s.contains("HEAD"))
        .collect();

    Ok(Json(GitBranchesResponse {
        current,
        local,
        remote,
    }))
}

/// POST /api/git/checkout — switch to a different branch.
pub async fn git_checkout(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<GitCheckoutRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Validate branch name (basic safety check)
    if req.branch.is_empty()
        || req.branch.contains("..")
        || req.branch.contains("~")
        || req.branch.starts_with('-')
    {
        return Err(WebError::BadRequest("Invalid branch name".to_string()));
    }

    let output = tokio::process::Command::new("git")
        .args(["checkout", &req.branch])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git checkout: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        Ok(Json(GitCheckoutResponse {
            branch: req.branch,
            success: true,
            message: if stderr.is_empty() { None } else { Some(stderr) },
        }))
    } else {
        Ok(Json(GitCheckoutResponse {
            branch: req.branch,
            success: false,
            message: Some(if stderr.is_empty() {
                "Checkout failed".to_string()
            } else {
                stderr
            }),
        }))
    }
}

/// GET /api/git/range-diff — get commit log + cumulative diff between base branch and HEAD.
///
/// Useful for "Draft PR Description" — gathers all commits and changes relative to a base branch.
pub async fn git_range_diff(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<GitRangeDiffQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);
    let base = query.base.unwrap_or_else(|| "main".to_string());
    let limit = query.limit.unwrap_or(50);

    // Get current branch
    let branch_out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git rev-parse: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();

    // Get commits in range base..HEAD
    let log_out = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("{}..HEAD", base),
            &format!("--max-count={}", limit),
            "--format=%H\x1f%h\x1f%an\x1f%aI\x1f%s",
        ])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git log: {e}")))?;

    let commits: Vec<GitLogEntry> = String::from_utf8_lossy(&log_out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, '\x1f').collect();
            if parts.len() == 5 {
                Some(GitLogEntry {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                    message: parts[4].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    // Get cumulative diff
    let diff_out = tokio::process::Command::new("git")
        .args(["diff", &format!("{}...HEAD", base)])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git diff: {e}")))?;
    let diff = String::from_utf8_lossy(&diff_out.stdout).to_string();

    // Count files changed
    let stat_out = tokio::process::Command::new("git")
        .args(["diff", &format!("{}...HEAD", base), "--stat"])
        .current_dir(dir_path)
        .output()
        .await
        .ok();
    let files_changed = stat_out
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| l.contains('|'))
                .count()
        })
        .unwrap_or(0);

    Ok(Json(GitRangeDiffResponse {
        branch,
        base,
        commits,
        diff,
        files_changed,
    }))
}

/// GET /api/git/context-summary — structured git context for AI injection.
///
/// Returns current branch, recent commits, change counts, and a human-readable
/// summary suitable for prepending to an AI session.
pub async fn git_context_summary(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let dir_path = std::path::Path::new(&dir);

    // Get current branch
    let branch_out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git rev-parse: {e}")))?;
    let branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();

    // Recent commits (last 5)
    let log_out = tokio::process::Command::new("git")
        .args([
            "log",
            "--max-count=5",
            "--format=%H\x1f%h\x1f%an\x1f%aI\x1f%s",
        ])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git log: {e}")))?;

    let recent_commits: Vec<GitLogEntry> = String::from_utf8_lossy(&log_out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(5, '\x1f').collect();
            if parts.len() == 5 {
                Some(GitLogEntry {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                    message: parts[4].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    // Get status counts
    let status_out = tokio::process::Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(dir_path)
        .output()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to run git status: {e}")))?;

    let status_text = String::from_utf8_lossy(&status_out.stdout);
    let mut staged_count = 0usize;
    let mut unstaged_count = 0usize;
    let mut untracked_count = 0usize;

    for line in status_text.lines() {
        if line.len() < 2 {
            continue;
        }
        let index = line.as_bytes()[0];
        let worktree = line.as_bytes()[1];

        if index == b'?' {
            untracked_count += 1;
        } else {
            if index != b' ' && index != b'?' {
                staged_count += 1;
            }
            if worktree != b' ' && worktree != b'?' {
                unstaged_count += 1;
            }
        }
    }

    // Build human-readable summary
    let mut summary_parts = vec![format!("Branch: {}", branch)];
    if !recent_commits.is_empty() {
        summary_parts.push(format!(
            "Last commit: {} ({})",
            recent_commits[0].message, recent_commits[0].short_hash
        ));
    }
    if staged_count > 0 {
        summary_parts.push(format!("{} file(s) staged", staged_count));
    }
    if unstaged_count > 0 {
        summary_parts.push(format!("{} file(s) modified (unstaged)", unstaged_count));
    }
    if untracked_count > 0 {
        summary_parts.push(format!("{} untracked file(s)", untracked_count));
    }
    if staged_count == 0 && unstaged_count == 0 && untracked_count == 0 {
        summary_parts.push("Working tree clean".to_string());
    }
    let summary = summary_parts.join(". ");

    Ok(Json(GitContextSummaryResponse {
        branch,
        recent_commits,
        staged_count,
        unstaged_count,
        untracked_count,
        summary,
    }))
}

// ── File browsing / editing endpoints ───────────────────────────────

/// GET /api/agents — list available agents.
///
/// Primary path: proxies `GET {opencode}/agent` to get the live, fully-resolved
/// agent list from the running opencode instance (same as opencode's own web UI).
///
/// Fallback: if the opencode instance is unreachable, reads the project's
/// `opencode.json` / `.opencode/config.json` for agent definitions and injects
/// built-in defaults.
pub async fn get_agents(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // ── Primary: query the running opencode instance ────────────────
    if let Ok(resp) = state.http_client
        .get(format!("{}/agent", base))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(upstream) = resp.json::<Vec<serde_json::Value>>().await {
                let agents: Vec<AgentEntry> = upstream
                    .iter()
                    .map(|v| AgentEntry {
                        id: v.get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string(),
                        label: {
                            let name = v.get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("");
                            // Capitalize first letter for display
                            let mut chars = name.chars();
                            match chars.next() {
                                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                                None => name.to_string(),
                            }
                        },
                        description: v.get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string(),
                        mode: v.get("mode")
                            .and_then(|m| m.as_str())
                            .unwrap_or("all")
                            .to_string(),
                        hidden: v.get("hidden")
                            .and_then(|h| h.as_bool())
                            .unwrap_or(false),
                        native: v.get("native")
                            .and_then(|n| n.as_bool())
                            .unwrap_or(false),
                        color: v.get("color")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string()),
                    })
                    .collect();

                if !agents.is_empty() {
                    return Ok(Json(agents));
                }
            }
        }
    }

    // ── Fallback: read static config files ──────────────────────────
    let dir_path = std::path::Path::new(&dir);
    let config_paths = [
        dir_path.join("opencode.json"),
        dir_path.join(".opencode/config.json"),
        dir_path.join(".opencode.json"),
    ];

    let mut agents = Vec::new();

    for config_path in &config_paths {
        if let Ok(content) = tokio::fs::read_to_string(config_path).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(agents_obj) = json.get("agents").and_then(|a| a.as_object()) {
                    for (id, agent_config) in agents_obj {
                        let description = agent_config
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let label = agent_config
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| {
                                let mut chars = id.chars();
                                match chars.next() {
                                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                                    None => id.clone(),
                                }
                            });
                        let mode = agent_config
                            .get("mode")
                            .and_then(|m| m.as_str())
                            .unwrap_or("all")
                            .to_string();
                        let hidden = agent_config
                            .get("hidden")
                            .and_then(|h| h.as_bool())
                            .unwrap_or(false);
                        let color = agent_config
                            .get("color")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string());
                        agents.push(AgentEntry {
                            id: id.clone(),
                            label,
                            description,
                            mode,
                            hidden,
                            native: false,
                            color,
                        });
                    }
                }
                break;
            }
        }
    }

    // Ensure built-in defaults (must match upstream opencode agent names)
    let has_build = agents.iter().any(|a| a.id == "build");
    let has_plan = agents.iter().any(|a| a.id == "plan");

    if !has_build {
        agents.insert(0, AgentEntry {
            id: "build".to_string(),
            label: "Build".to_string(),
            description: "Default coding agent".to_string(),
            mode: "primary".to_string(),
            hidden: false,
            native: true,
            color: None,
        });
    }
    if !has_plan {
        agents.push(AgentEntry {
            id: "plan".to_string(),
            label: "Plan".to_string(),
            description: "Planning and design agent".to_string(),
            mode: "all".to_string(),
            hidden: false,
            native: true,
            color: None,
        });
    }

    for agent in &mut agents {
        if agent.id == "build" || agent.id == "plan" {
            agent.native = true;
        }
    }

    agents.sort_by(|a, b| {
        let order = |id: &str| -> usize {
            match id {
                "build" => 0,
                "plan" => 1,
                _ => 2,
            }
        };
        order(&a.id).cmp(&order(&b.id)).then_with(|| a.id.cmp(&b.id))
    });

    Ok(Json(agents))
}

/// GET /api/files?path=... — list directory contents.
pub async fn browse_files(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileBrowseQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let rel = if query.path.is_empty() {
        ".".to_string()
    } else {
        query.path.clone()
    };
    let target = base.join(&rel);

    // Security: ensure resolved path is within project dir
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("Directory not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    let mut entries = Vec::new();
    let mut dir_reader = tokio::fs::read_dir(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read directory: {e}")))?;

    while let Some(entry) = dir_reader
        .next_entry()
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read entry: {e}")))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs (starting with .)
        if name.starts_with('.') {
            continue;
        }
        let metadata = entry
            .metadata()
            .await
            .map_err(|e| WebError::Internal(format!("Failed to read metadata: {e}")))?;
        let entry_path = if rel == "." {
            name.clone()
        } else {
            format!("{}/{}", rel, name)
        };
        entries.push(FileEntry {
            name,
            path: entry_path,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
        });
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(FileBrowseResponse {
        path: rel,
        entries,
    }))
}

/// GET /api/file/read?path=... — read file content.
pub async fn read_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&query.path);

    // Security check
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("File not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    let content = tokio::fs::read_to_string(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read file: {e}")))?;

    let language = detect_language(&query.path);

    Ok(Json(FileReadResponse {
        path: query.path,
        content,
        language,
    }))
}

/// GET /api/file/raw?path=... — serve raw file bytes with Content-Type.
/// Used for binary files (images, audio, video, PDFs, etc).
pub async fn read_file_raw(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<FileReadQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&query.path);

    // Security check
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|_| WebError::NotFound("File not found"))?;
    if !canonical_target.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    let bytes = tokio::fs::read(&canonical_target)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read file: {e}")))?;

    let content_type = mime_from_extension(&query.path);

    Ok((
        [(axum::http::header::CONTENT_TYPE, content_type)],
        bytes,
    ))
}

/// Map file extension to MIME type for binary file serving.
fn mime_from_extension(path: &str) -> String {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        // Images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        // Audio
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "weba" => "audio/webm",
        // Video
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        // Documents
        "pdf" => "application/pdf",
        "csv" => "text/csv",
        "xlsx" | "xls" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" | "ppt" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "docx" | "doc" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        // Fallback
        _ => "application/octet-stream",
    }
    .to_string()
}

/// POST /api/file/write — write file content.
pub async fn write_file(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<FileWriteRequest>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = std::path::Path::new(&dir);
    let target = base.join(&req.path);

    // Security check — for writes, we can't canonicalize if file doesn't exist yet,
    // so we canonicalize the parent instead
    let canonical_base = base
        .canonicalize()
        .map_err(|e| WebError::Internal(format!("Failed to resolve base: {e}")))?;
    let parent = target.parent().ok_or(WebError::BadRequest(
        "Invalid file path".into(),
    ))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| WebError::NotFound("Parent directory not found"))?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(WebError::BadRequest("Path traversal not allowed".into()));
    }

    tokio::fs::write(&target, &req.content)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to write file: {e}")))?;

    Ok(StatusCode::OK)
}

pub async fn editor_lsp_diagnostics(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (socket, _resolved, buf) = resolve_editor_buffer(&state, &query.session_id, &query.path).await?;
    let raw = crate::nvim_rpc::nvim_lsp_diagnostics(&socket, buf, true)
        .map_err(|e| WebError::Internal(format!("Failed to get diagnostics: {e}")))?;
    let diagnostics: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!([]));
    Ok(Json(serde_json::json!({
        "available": true,
        "diagnostics": diagnostics,
    })))
}

pub async fn editor_lsp_hover(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (socket, _resolved, buf) = resolve_editor_buffer(&state, &query.session_id, &query.path).await?;
    let raw = crate::nvim_rpc::nvim_lsp_hover(&socket, buf, query.line, query.col)
        .map_err(|e| WebError::Internal(format!("Failed to get hover: {e}")))?;
    let hover = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(v) if v.get("error").is_some() => None,
        _ => Some(raw),
    };
    Ok(Json(serde_json::json!({
        "available": true,
        "hover": hover,
    })))
}

pub async fn editor_lsp_definition(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<EditorLspQuery>,
) -> WebResult<impl IntoResponse> {
    let (socket, _resolved, buf) = resolve_editor_buffer(&state, &query.session_id, &query.path).await?;
    let raw = crate::nvim_rpc::nvim_lsp_definition(&socket, buf, query.line, query.col)
        .map_err(|e| WebError::Internal(format!("Failed to get definition: {e}")))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    let locations = parsed
        .get("locations")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok(Json(serde_json::json!({
        "available": true,
        "locations": locations,
    })))
}

pub async fn editor_lsp_format(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<EditorFormatRequest>,
) -> WebResult<impl IntoResponse> {
    let (socket, resolved, buf) = resolve_editor_buffer(&state, &req.session_id, &req.path).await?;
    let _ = crate::nvim_rpc::nvim_lsp_format(&socket, buf)
        .map_err(|e| WebError::Internal(format!("Failed to format file: {e}")))?;
    let _ = crate::nvim_rpc::nvim_write(&socket, buf, false);
    let content = tokio::fs::read_to_string(&resolved)
        .await
        .map_err(|e| WebError::Internal(format!("Failed to read formatted file: {e}")))?;
    Ok(Json(serde_json::json!({
        "available": true,
        "formatted": true,
        "content": content,
    })))
}

/// Detect language from file extension for CodeMirror syntax highlighting.
fn detect_language(path: &str) -> String {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "py" | "pyw" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "cpp",
        "json" => "json",
        "html" | "htm" => "html",
        "css" | "scss" | "less" => "css",
        "md" | "mdx" | "markdown" => "markdown",
        "sql" => "sql",
        "xml" | "svg" => "xml",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" => "shell",
        "fish" => "shell",
        "lua" => "lua",
        "rb" => "ruby",
        "php" => "php",
        "vue" => "vue",
        "svelte" => "svelte",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "mmd" | "mermaid" => "mermaid",
        "ini" | "cfg" | "conf" => "ini",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "diff" | "patch" => "diff",
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        _ => "text",
    }
    .to_string()
}

/// GET /api/session/:id/todos — fetch todos for a session.
pub async fn get_session_todos(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();
    let client = ApiClient::with_client(state.http_client.clone());
    let todos = client
        .fetch_todos(&base, &dir, &session_id)
        .await
        .map_err(|e| WebError::Internal(format!("{e}")))?;
    Ok(Json(todos))
}

// ── Watcher API endpoints ───────────────────────────────────────────

/// GET /api/watchers — list all active watchers with real-time status.
pub async fn list_watchers(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let watchers = state.web_state.list_watchers().await;
    Ok(Json(watchers))
}

/// POST /api/watcher — create or update a watcher for a session.
pub async fn create_watcher(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<WatcherConfigRequest>,
) -> WebResult<impl IntoResponse> {
    if req.session_id.is_empty() {
        return Err(WebError::BadRequest("session_id is required".into()));
    }
    if req.continuation_message.trim().is_empty() {
        return Err(WebError::BadRequest("continuation_message is required".into()));
    }
    if req.idle_timeout_secs == 0 {
        return Err(WebError::BadRequest("idle_timeout_secs must be > 0".into()));
    }
    let response = state.web_state.create_watcher(req).await;
    Ok(Json(response))
}

/// DELETE /api/watcher/:session_id — remove a watcher.
pub async fn delete_watcher(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    if state.web_state.delete_watcher(&session_id).await {
        Ok(StatusCode::OK)
    } else {
        Err(WebError::NotFound("No watcher found for this session"))
    }
}

/// GET /api/watcher/:session_id — get watcher config and status for a session.
pub async fn get_watcher(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    match state.web_state.get_watcher(&session_id).await {
        Some(w) => Ok(Json(w)),
        None => Err(WebError::NotFound("No watcher found for this session")),
    }
}

// ── Context Window endpoint ─────────────────────────────────────────

/// Query params for `GET /api/context-window?session_id=...`.
#[derive(serde::Deserialize)]
pub struct ContextWindowQuery {
    /// Session ID to get context usage for.
    pub session_id: Option<String>,
}

/// GET /api/context-window — get context window usage breakdown.
///
/// Returns the context limit for the active model and a breakdown of
/// token usage by category (input, output, reasoning, cache).
pub async fn get_context_window(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Query(query): axum::extract::Query<ContextWindowQuery>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // 1. Determine which session to inspect
    let session_id = match query.session_id {
        Some(sid) => sid,
        None => state
            .web_state
            .active_session_id()
            .await
            .ok_or(WebError::BadRequest("No active session".into()))?,
    };

    // 2. Get session stats (already tracked via SSE)
    let stats = state
        .web_state
        .get_session_stats(&session_id)
        .await
        .unwrap_or_default();

    let total_used = stats.input_tokens
        + stats.output_tokens
        + stats.reasoning_tokens
        + stats.cache_read
        + stats.cache_write;

    // 3. Get context limit from providers
    let context_limit = {
        let client = ApiClient::with_client(state.http_client.clone());
        // Fetch providers to find the max context window
        let providers_result = client.fetch_providers(&base, &dir).await;
        match providers_result {
            Ok(providers_val) => {
                // providers_val is a serde_json::Value
                // Extract the default model's context limit, or find the max
                let mut max_context: u64 = 0;
                if let Some(all) = providers_val.get("all").and_then(|v| v.as_array()) {
                    for provider in all {
                        if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                            for (_model_id, model_info) in models {
                                if let Some(ctx) = model_info
                                    .pointer("/limit/context")
                                    .and_then(|c| c.as_u64())
                                {
                                    if ctx > max_context {
                                        max_context = ctx;
                                    }
                                }
                            }
                        }
                    }
                }
                // Also try the flat array format
                if max_context == 0 {
                    if let Some(arr) = providers_val.as_array() {
                        for provider in arr {
                            if let Some(models) = provider.get("models").and_then(|m| m.as_object()) {
                                for (_model_id, model_info) in models {
                                    if let Some(ctx) = model_info
                                        .pointer("/limit/context")
                                        .and_then(|c| c.as_u64())
                                    {
                                        if ctx > max_context {
                                            max_context = ctx;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if max_context > 0 { max_context } else { 200_000 }
            }
            Err(_) => 200_000, // Fallback
        }
    };

    let usage_pct = if context_limit > 0 {
        (total_used as f64 / context_limit as f64) * 100.0
    } else {
        0.0
    };

    // 4. Build category breakdown from stats
    let mut categories = Vec::new();

    if stats.input_tokens > 0 {
        categories.push(ContextCategory {
            name: "input".to_string(),
            label: "Input Tokens".to_string(),
            tokens: stats.input_tokens,
            pct: if context_limit > 0 { (stats.input_tokens as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "blue".to_string(),
            items: vec![],
        });
    }

    if stats.output_tokens > 0 {
        categories.push(ContextCategory {
            name: "output".to_string(),
            label: "Output Tokens".to_string(),
            tokens: stats.output_tokens,
            pct: if context_limit > 0 { (stats.output_tokens as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "green".to_string(),
            items: vec![],
        });
    }

    if stats.reasoning_tokens > 0 {
        categories.push(ContextCategory {
            name: "reasoning".to_string(),
            label: "Reasoning Tokens".to_string(),
            tokens: stats.reasoning_tokens,
            pct: if context_limit > 0 { (stats.reasoning_tokens as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "purple".to_string(),
            items: vec![],
        });
    }

    if stats.cache_read > 0 || stats.cache_write > 0 {
        let cache_total = stats.cache_read + stats.cache_write;
        let mut items = Vec::new();
        if stats.cache_read > 0 {
            items.push(ContextItem {
                label: "Cache Read".to_string(),
                tokens: stats.cache_read,
            });
        }
        if stats.cache_write > 0 {
            items.push(ContextItem {
                label: "Cache Write".to_string(),
                tokens: stats.cache_write,
            });
        }
        categories.push(ContextCategory {
            name: "cache".to_string(),
            label: "Cache".to_string(),
            tokens: cache_total,
            pct: if context_limit > 0 { (cache_total as f64 / context_limit as f64) * 100.0 } else { 0.0 },
            color: "gray".to_string(),
            items,
        });
    }

    // 5. Estimate remaining messages
    // Use average tokens per message pair to estimate remaining capacity
    let estimated_messages_remaining = if total_used > 0 && context_limit > total_used {
        // Fetch message count to calculate average
        let remaining = context_limit - total_used;
        // Rough heuristic: count messages via the stats
        // Average input per exchange ~ input_tokens / max(1, number_of_exchanges)
        // Since we don't have message count here, use a simple heuristic
        let avg_per_exchange = if stats.input_tokens > 0 {
            // Assume input_tokens is split across ~N exchanges, each response
            // generates roughly equal output. Simple estimate: total / 2
            total_used / 2 // very rough: each exchange = total_so_far / messages
        } else {
            10_000 // default assumption: 10K tokens per exchange
        };
        if avg_per_exchange > 0 {
            Some(remaining / avg_per_exchange)
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(ContextWindowResponse {
        context_limit,
        total_used,
        usage_pct,
        categories,
        estimated_messages_remaining,
    }))
}

/// GET /api/watcher/sessions — list all sessions formatted for the watcher session picker.
pub async fn get_watcher_sessions(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let sessions = state.web_state.get_watcher_sessions().await;
    Ok(Json(sessions))
}

/// GET /api/watcher/:session_id/messages — fetch user messages from a session
/// for the "re-inject original message" picker.
pub async fn get_watcher_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // Fetch messages from the opencode server
    let resp = state.http_client
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

    // Extract user messages only
    let all_messages: Vec<serde_json::Value> = if let Some(arr) = body.as_array() {
        arr.clone()
    } else if let Some(obj) = body.as_object() {
        obj.values().cloned().collect()
    } else {
        vec![]
    };

    let mut user_messages: Vec<WatcherMessageEntry> = Vec::new();
    for msg in &all_messages {
        let role = msg.pointer("/info/role")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if role != "user" {
            continue;
        }
        // Extract text from parts
        let parts = msg.get("parts").and_then(|v| v.as_array());
        if let Some(parts) = parts {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                    if !text.trim().is_empty() {
                        user_messages.push(WatcherMessageEntry {
                            role: "user".to_string(),
                            text: text.to_string(),
                        });
                    }
                }
            }
        }
    }

    // Reverse so most recent is first
    user_messages.reverse();

    Ok(Json(user_messages))
}

// ── File Edits / Diff Review ────────────────────────────────────────

/// GET /api/session/{session_id}/file-edits
///
/// Returns all file edits tracked for a session (original and new content).
pub async fn get_file_edits(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> WebResult<impl IntoResponse> {
    let edits = state.web_state.get_file_edits(&session_id).await;

    // Deduplicate: only keep the latest edit per file path
    let mut latest_by_path: std::collections::HashMap<String, &super::web_state::FileEditRecord> =
        std::collections::HashMap::new();
    for edit in &edits {
        latest_by_path.insert(edit.path.clone(), edit);
    }

    let mut result: Vec<FileEditEntry> = latest_by_path
        .into_values()
        .map(|e| FileEditEntry {
            path: e.path.clone(),
            original_content: e.original_content.clone(),
            new_content: e.new_content.clone(),
            timestamp: e.timestamp.clone(),
            index: e.index,
        })
        .collect();
    result.sort_by_key(|e| e.index);

    let file_count = result.len();

    Ok(Json(FileEditsResponse {
        session_id,
        edits: result,
        file_count,
    }))
}

// ── Cross-Session Search ────────────────────────────────────────────

/// Query parameters for the search endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    50
}

/// GET /api/project/{idx}/search?q=<query>&limit=50
///
/// Searches across all sessions in a project by fetching messages from the
/// opencode API and doing case-insensitive substring matching on message text,
/// tool call names, arguments, and output.
pub async fn search_messages(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(project_idx): axum::extract::Path<usize>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> WebResult<impl IntoResponse> {
    let query = params.q.trim().to_string();
    if query.is_empty() {
        return Ok(Json(SearchResponse {
            query,
            results: vec![],
            total: 0,
        }));
    }

    let (project_path, project_name, sessions) = state
        .web_state
        .get_project_sessions(project_idx)
        .await
        .ok_or(WebError::BadRequest("Invalid project index".into()))?;

    let base = base_url().to_string();
    let dir = project_path.to_string_lossy().to_string();
    let query_lower = query.to_lowercase();
    let limit = params.limit.min(200); // cap at 200

    let mut results: Vec<SearchResultEntry> = Vec::new();

    // Search each session's messages
    for (session_id, session_title) in &sessions {
        if results.len() >= limit {
            break;
        }

        // Fetch messages for this session
        let resp = match state
            .http_client
            .get(format!("{}/session/{}/message", base, session_id))
            .header("x-opencode-directory", &dir)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Normalise into flat Vec
        let messages: Vec<&serde_json::Value> = if let Some(arr) = body.as_array() {
            arr.iter().collect()
        } else if let Some(obj) = body.as_object() {
            obj.values().collect()
        } else {
            continue;
        };

        for msg in &messages {
            if results.len() >= limit {
                break;
            }

            let role = msg
                .pointer("/info/role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let msg_id = msg
                .pointer("/info/id")
                .or_else(|| msg.pointer("/info/messageID"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let timestamp = msg
                .pointer("/info/time/created")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            // Collect all searchable text from parts
            let parts = msg.get("parts").and_then(|v| v.as_array());
            if let Some(parts) = parts {
                for part in parts {
                    let mut searchable_texts: Vec<&str> = Vec::new();

                    // Text content
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        searchable_texts.push(text);
                    }

                    // Tool call name
                    if let Some(name) = part.get("toolName").and_then(|v| v.as_str()) {
                        searchable_texts.push(name);
                    }

                    // Tool call args (stringify)
                    if let Some(args) = part.get("args") {
                        if let Some(s) = args.as_str() {
                            searchable_texts.push(s);
                        }
                    }

                    // Tool call output/result
                    if let Some(output) = part.get("output").and_then(|v| v.as_str()) {
                        searchable_texts.push(output);
                    }
                    if let Some(result) = part.get("result").and_then(|v| v.as_str()) {
                        searchable_texts.push(result);
                    }

                    // Check if any text matches
                    for text in &searchable_texts {
                        if text.to_lowercase().contains(&query_lower) {
                            // Build snippet: find match position and extract context
                            let snippet = build_snippet(text, &query_lower, 120);
                            results.push(SearchResultEntry {
                                session_id: session_id.clone(),
                                session_title: session_title.clone(),
                                project_name: project_name.clone(),
                                message_id: msg_id.clone(),
                                role: role.to_string(),
                                snippet,
                                timestamp,
                            });
                            break; // one match per message is enough
                        }
                    }

                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    let total = results.len();
    Ok(Json(SearchResponse {
        query,
        results,
        total,
    }))
}

/// Build a snippet around the first occurrence of `needle` in `haystack`.
/// Returns at most `max_len` characters with "..." ellipsis if truncated.
fn build_snippet(haystack: &str, needle_lower: &str, max_len: usize) -> String {
    let lower = haystack.to_lowercase();
    let pos = match lower.find(needle_lower) {
        Some(p) => p,
        None => return haystack.chars().take(max_len).collect(),
    };

    // Compute a window around the match
    let context = max_len / 2;
    let start = pos.saturating_sub(context);
    let end = (pos + needle_lower.len() + context).min(haystack.len());

    // Adjust to char boundaries
    let start = haystack
        .char_indices()
        .find(|(i, _)| *i >= start)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = haystack
        .char_indices()
        .rev()
        .find(|(i, _)| *i <= end)
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(haystack.len());

    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(&haystack[start..end]);
    if end < haystack.len() {
        snippet.push_str("...");
    }
    snippet
}

// ── Multi-session dashboard handlers ─────────────────────────────────

/// GET /api/sessions/overview — flat list of all sessions across all projects
/// with status, cost, and timing info.
pub async fn sessions_overview(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let overview = state.web_state.get_sessions_overview().await;
    Json(overview)
}

/// GET /api/sessions/tree — hierarchical parent/child session tree.
pub async fn sessions_tree(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let tree = state.web_state.get_sessions_tree().await;
    Json(tree)
}

// ── Session Continuity: Presence + Activity ─────────────────────────

/// GET /api/presence — get current connected clients.
pub async fn get_presence(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let snapshot = state.web_state.get_presence().await;
    Json(super::types::PresenceResponse {
        clients: snapshot.clients,
    })
}

/// POST /api/presence — register or update client presence.
pub async fn register_presence(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::PresenceRegisterRequest>,
) -> impl IntoResponse {
    let now = chrono::Utc::now().to_rfc3339();
    let presence = super::types::ClientPresence {
        client_id: req.client_id,
        interface_type: req.interface_type,
        focused_session: req.focused_session,
        last_seen: now,
    };
    state.web_state.register_presence(&presence).await;
    StatusCode::OK
}

/// DELETE /api/presence — deregister client presence.
pub async fn deregister_presence(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::PresenceDeregisterRequest>,
) -> impl IntoResponse {
    state.web_state.deregister_presence(&req.client_id).await;
    StatusCode::OK
}

/// Query for activity feed endpoint.
#[derive(serde::Deserialize)]
pub struct ActivityFeedQuery {
    pub session_id: String,
}

/// GET /api/activity — get recent activity events for a session.
pub async fn get_activity_feed(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(params): Query<ActivityFeedQuery>,
) -> impl IntoResponse {
    let events = state.web_state.get_activity_feed(&params.session_id).await;
    Json(super::types::ActivityFeedResponse {
        session_id: params.session_id,
        events,
    })
}

// ── Missions ────────────────────────────────────────────────────────

/// GET /api/missions — list all saved missions.
pub async fn list_missions(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let missions = state.web_state.list_missions().await;
    Json(super::types::MissionsListResponse { missions })
}

/// POST /api/missions — create a mission.
pub async fn create_mission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::CreateMissionRequest>,
) -> impl IntoResponse {
    let mission = state.web_state.create_mission(req).await;
    (StatusCode::CREATED, Json(mission))
}

/// PATCH /api/missions/{mission_id} — update an existing mission.
pub async fn update_mission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(mission_id): axum::extract::Path<String>,
    Json(req): Json<super::types::UpdateMissionRequest>,
) -> impl IntoResponse {
    match state.web_state.update_mission(&mission_id, req).await {
        Some(mission) => (StatusCode::OK, Json(mission)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/missions/{mission_id} — delete a mission.
pub async fn delete_mission(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(mission_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_mission(&mission_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// GET /api/memory — list all personal memory items.
pub async fn list_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let memory = state.web_state.list_personal_memory().await;
    Json(super::types::PersonalMemoryListResponse { memory })
}

/// POST /api/memory — create a personal memory item.
pub async fn create_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::CreatePersonalMemoryRequest>,
) -> impl IntoResponse {
    let item = state.web_state.create_personal_memory(req).await;
    (StatusCode::CREATED, Json(item))
}

/// PATCH /api/memory/{memory_id} — update a memory item.
pub async fn update_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(memory_id): axum::extract::Path<String>,
    Json(req): Json<super::types::UpdatePersonalMemoryRequest>,
) -> impl IntoResponse {
    match state.web_state.update_personal_memory(&memory_id, req).await {
        Some(item) => (StatusCode::OK, Json(item)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/memory/{memory_id} — delete a memory item.
pub async fn delete_personal_memory(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(memory_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_personal_memory(&memory_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// GET /api/autonomy — get autonomy settings.
pub async fn get_autonomy_settings(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    Json(state.web_state.get_autonomy_settings().await)
}

/// POST /api/autonomy — update autonomy settings.
pub async fn update_autonomy_settings(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::UpdateAutonomySettingsRequest>,
) -> impl IntoResponse {
    Json(state.web_state.update_autonomy_settings(req.mode).await)
}

/// GET /api/routines — list routines and run history.
pub async fn list_routines(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let (routines, runs) = state.web_state.list_routines().await;
    Json(super::types::RoutinesListResponse { routines, runs })
}

/// POST /api/routines — create a routine.
pub async fn create_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::CreateRoutineRequest>,
) -> impl IntoResponse {
    let routine = state.web_state.create_routine(req).await;
    (StatusCode::CREATED, Json(routine))
}

/// PATCH /api/routines/{routine_id} — update a routine.
pub async fn update_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
    Json(req): Json<super::types::UpdateRoutineRequest>,
) -> impl IntoResponse {
    match state.web_state.update_routine(&routine_id, req).await {
        Some(routine) => (StatusCode::OK, Json(routine)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/routines/{routine_id} — delete a routine.
pub async fn delete_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_routine(&routine_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// POST /api/routines/{routine_id}/run — record a manual run.
pub async fn run_routine(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(routine_id): axum::extract::Path<String>,
    Json(req): Json<super::types::RunRoutineRequest>,
) -> impl IntoResponse {
    Json(
        state
            .web_state
            .record_routine_run(
                &routine_id,
                req.summary.unwrap_or_else(|| "Routine executed manually".to_string()),
            )
            .await,
    )
}

/// GET /api/delegation — list delegated work items.
pub async fn list_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    Json(super::types::DelegatedWorkListResponse {
        items: state.web_state.list_delegated_work().await,
    })
}

/// POST /api/delegation — create delegated work item.
pub async fn create_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::CreateDelegatedWorkRequest>,
) -> impl IntoResponse {
    let item = state.web_state.create_delegated_work(req).await;
    (StatusCode::CREATED, Json(item))
}

/// PATCH /api/delegation/{item_id} — update delegated work item.
pub async fn update_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(item_id): axum::extract::Path<String>,
    Json(req): Json<super::types::UpdateDelegatedWorkRequest>,
) -> impl IntoResponse {
    match state.web_state.update_delegated_work(&item_id, req).await {
        Some(item) => (StatusCode::OK, Json(item)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// DELETE /api/delegation/{item_id} — delete delegated work item.
pub async fn delete_delegated_work(
    State(state): State<ServerState>,
    _auth: AuthUser,
    axum::extract::Path(item_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if state.web_state.delete_delegated_work(&item_id).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

// ── Workspace Snapshots ─────────────────────────────────────────────

/// GET /api/workspaces — list all saved workspace snapshots.
pub async fn list_workspaces(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let workspaces = state.web_state.list_workspaces().await;
    Json(super::types::WorkspacesListResponse { workspaces })
}

/// POST /api/workspaces — save (upsert) a workspace snapshot.
pub async fn save_workspace(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(req): Json<super::types::SaveWorkspaceRequest>,
) -> impl IntoResponse {
    state.web_state.save_workspace(req.snapshot).await;
    StatusCode::OK
}

/// Query param for workspace deletion by name.
#[derive(serde::Deserialize)]
pub struct WorkspaceDeleteQuery {
    pub name: String,
}

/// DELETE /api/workspaces — delete a workspace snapshot by name.
pub async fn delete_workspace(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(params): Query<WorkspaceDeleteQuery>,
) -> impl IntoResponse {
    if state.web_state.delete_workspace(&params.name).await {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

// ═══════════════════════════════════════════════════════════════════
// Unit tests for pure / private helper functions
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── constant_time_eq ────────────────────────────────────────

    #[test]
    fn cte_equal_slices() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn cte_empty_slices() {
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn cte_different_content() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn cte_different_lengths() {
        assert!(!constant_time_eq(b"short", b"longer string"));
    }

    #[test]
    fn cte_single_bit_diff() {
        // 'A' = 0x41, 'B' = 0x42 — differ by one bit
        assert!(!constant_time_eq(b"A", b"B"));
    }

    #[test]
    fn cte_binary_data() {
        let a = vec![0u8, 1, 2, 255, 128];
        let b = vec![0u8, 1, 2, 255, 128];
        assert!(constant_time_eq(&a, &b));
        let mut c = b.clone();
        c[4] = 127;
        assert!(!constant_time_eq(&a, &c));
    }

    // ── mime_from_extension ─────────────────────────────────────

    #[test]
    fn mime_images() {
        assert_eq!(mime_from_extension("photo.png"), "image/png");
        assert_eq!(mime_from_extension("photo.PNG"), "image/png");
        assert_eq!(mime_from_extension("photo.jpg"), "image/jpeg");
        assert_eq!(mime_from_extension("photo.jpeg"), "image/jpeg");
        assert_eq!(mime_from_extension("anim.gif"), "image/gif");
        assert_eq!(mime_from_extension("icon.svg"), "image/svg+xml");
        assert_eq!(mime_from_extension("pic.webp"), "image/webp");
        assert_eq!(mime_from_extension("fav.ico"), "image/x-icon");
        assert_eq!(mime_from_extension("img.avif"), "image/avif");
    }

    #[test]
    fn mime_audio() {
        assert_eq!(mime_from_extension("song.mp3"), "audio/mpeg");
        assert_eq!(mime_from_extension("clip.wav"), "audio/wav");
        assert_eq!(mime_from_extension("track.ogg"), "audio/ogg");
        assert_eq!(mime_from_extension("music.flac"), "audio/flac");
        assert_eq!(mime_from_extension("a.m4a"), "audio/mp4");
    }

    #[test]
    fn mime_video() {
        assert_eq!(mime_from_extension("vid.mp4"), "video/mp4");
        assert_eq!(mime_from_extension("v.webm"), "video/webm");
        assert_eq!(mime_from_extension("m.mov"), "video/quicktime");
        assert_eq!(mime_from_extension("m.mkv"), "video/x-matroska");
    }

    #[test]
    fn mime_documents() {
        assert_eq!(mime_from_extension("doc.pdf"), "application/pdf");
        assert_eq!(mime_from_extension("data.csv"), "text/csv");
    }

    #[test]
    fn mime_unknown_falls_back() {
        assert_eq!(
            mime_from_extension("file.xyz"),
            "application/octet-stream"
        );
        assert_eq!(
            mime_from_extension("noext"),
            "application/octet-stream"
        );
    }

    #[test]
    fn mime_case_insensitive() {
        assert_eq!(mime_from_extension("FILE.PDF"), "application/pdf");
        assert_eq!(mime_from_extension("song.MP3"), "audio/mpeg");
    }

    // ── detect_language ─────────────────────────────────────────

    #[test]
    fn detect_rust() {
        assert_eq!(detect_language("main.rs"), "rust");
    }

    #[test]
    fn detect_javascript_variants() {
        assert_eq!(detect_language("app.js"), "javascript");
        assert_eq!(detect_language("App.jsx"), "javascript");
        assert_eq!(detect_language("index.mjs"), "javascript");
        assert_eq!(detect_language("config.cjs"), "javascript");
    }

    #[test]
    fn detect_typescript_variants() {
        assert_eq!(detect_language("app.ts"), "typescript");
        assert_eq!(detect_language("App.tsx"), "typescript");
        assert_eq!(detect_language("index.mts"), "typescript");
    }

    #[test]
    fn detect_python() {
        assert_eq!(detect_language("script.py"), "python");
        assert_eq!(detect_language("gui.pyw"), "python");
    }

    #[test]
    fn detect_various_languages() {
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("Main.java"), "java");
        assert_eq!(detect_language("lib.c"), "c");
        assert_eq!(detect_language("lib.h"), "c");
        assert_eq!(detect_language("lib.cpp"), "cpp");
        assert_eq!(detect_language("data.json"), "json");
        assert_eq!(detect_language("page.html"), "html");
        assert_eq!(detect_language("style.css"), "css");
        assert_eq!(detect_language("readme.md"), "markdown");
        assert_eq!(detect_language("query.sql"), "sql");
        assert_eq!(detect_language("layout.xml"), "xml");
        assert_eq!(detect_language("config.yaml"), "yaml");
        assert_eq!(detect_language("Cargo.toml"), "toml");
        assert_eq!(detect_language("run.sh"), "shell");
        assert_eq!(detect_language("init.lua"), "lua");
        assert_eq!(detect_language("app.rb"), "ruby");
        assert_eq!(detect_language("index.php"), "php");
    }

    #[test]
    fn detect_case_insensitive() {
        assert_eq!(detect_language("FILE.RS"), "rust");
        assert_eq!(detect_language("APP.TSX"), "typescript");
    }

    #[test]
    fn detect_unknown_falls_back() {
        assert_eq!(detect_language("file.xyz"), "text");
        assert_eq!(detect_language("noext"), "text");
    }

    // ── build_snippet ───────────────────────────────────────────

    #[test]
    fn snippet_basic_match() {
        let text = "The quick brown fox jumps over the lazy dog";
        let snippet = build_snippet(text, "fox", 30);
        assert!(snippet.contains("fox"));
        assert!(snippet.len() <= 40); // 30 + possible "..." * 2
    }

    #[test]
    fn snippet_no_match() {
        let text = "Hello world";
        let snippet = build_snippet(text, "xyz", 20);
        // Should return first max_len chars
        assert_eq!(snippet, "Hello world");
    }

    #[test]
    fn snippet_match_at_start() {
        let text = "Hello world, this is a longer text";
        let snippet = build_snippet(text, "hello", 20);
        assert!(snippet.starts_with("Hello"));
    }

    #[test]
    fn snippet_match_at_end() {
        let text = "This is a very long text that ends with target";
        let snippet = build_snippet(text, "target", 20);
        assert!(snippet.contains("target"));
    }

    #[test]
    fn snippet_short_text_returned_fully() {
        let text = "tiny";
        let snippet = build_snippet(text, "tiny", 100);
        assert_eq!(snippet, "tiny");
    }

    #[test]
    fn snippet_ellipsis_when_truncated() {
        // Place needle in the middle
        let mut haystack = "B".repeat(200);
        haystack.push_str("needle");
        haystack.push_str(&"B".repeat(200));
        let snippet = build_snippet(&haystack, "needle", 40);
        assert!(snippet.contains("needle"));
        // Should have ellipsis on at least one side
        assert!(snippet.contains("..."));
    }

    #[test]
    fn snippet_case_insensitive_needle() {
        let text = "Hello World";
        let snippet = build_snippet(text, "hello", 50);
        assert!(snippet.contains("Hello"));
    }

    // ── default_search_limit ────────────────────────────────────

    #[test]
    fn default_search_limit_is_50() {
        assert_eq!(default_search_limit(), 50);
    }
}
