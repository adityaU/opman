//! Authenticated `/api/browser/*` routes — the pane's half of the browser widget.
//!
//! Thin by design: every route delegates to [`super::browser_ops`], which the loopback
//! MCP routes call too.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::WebResult;
use super::super::types::ServerState;
use super::browser_ops as ops;

#[derive(serde::Deserialize)]
pub struct OpenRequest {
    pub pane_id: String,
    /// The project this browser belongs to; browsers are per project.
    #[serde(default)]
    pub project: String,
    pub url: Option<String>,
}

pub async fn browser_open(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<OpenRequest>,
) -> WebResult<impl IntoResponse> {
    ops::open(
        &state,
        &request.pane_id,
        &request.project,
        request.url.as_deref(),
    )
    .await
    .map(Json)
}

pub async fn browser_navigate(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::NavigateRequest>,
) -> WebResult<impl IntoResponse> {
    ops::navigate(&state, &request.pane_id, &request.project, &request.url)
        .await
        .map(Json)
}

pub async fn browser_back(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::PaneRef>,
) -> WebResult<impl IntoResponse> {
    ops::step(&state, &request.pane_id, ops::Step::Back)
        .await
        .map(Json)
}

pub async fn browser_forward(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::PaneRef>,
) -> WebResult<impl IntoResponse> {
    ops::step(&state, &request.pane_id, ops::Step::Forward)
        .await
        .map(Json)
}

pub async fn browser_reload(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::PaneRef>,
) -> WebResult<impl IntoResponse> {
    ops::step(&state, &request.pane_id, ops::Step::Reload)
        .await
        .map(Json)
}

pub async fn browser_snapshot(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<ops::SnapshotQuery>,
) -> WebResult<impl IntoResponse> {
    ops::snapshot(&state, &query.pane_id, query.options)
        .await
        .map(Json)
}

pub async fn browser_text(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<ops::TextQuery>,
) -> WebResult<impl IntoResponse> {
    ops::read_text(&state, &query.pane_id, query.max_chars)
        .await
        .map(Json)
}

pub async fn browser_screenshot(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Query(query): Query<ops::ScreenshotQuery>,
) -> WebResult<impl IntoResponse> {
    ops::screenshot(&state, &query.pane_id, query.quality)
        .await
        .map(Json)
}

pub async fn browser_click(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::ClickRequest>,
) -> WebResult<impl IntoResponse> {
    ops::click(&state, &request).await.map(Json)
}

pub async fn browser_type(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::TypeRequest>,
) -> WebResult<impl IntoResponse> {
    ops::type_text(&state, &request).await.map(Json)
}

pub async fn browser_key(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::KeyRequest>,
) -> WebResult<impl IntoResponse> {
    ops::press_key(&state, &request).await.map(Json)
}

pub async fn browser_scroll(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::ScrollRequest>,
) -> WebResult<impl IntoResponse> {
    ops::scroll(&state, &request).await.map(Json)
}

pub async fn browser_mouse(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::MouseRequest>,
) -> WebResult<impl IntoResponse> {
    ops::mouse(&state, &request).await.map(Json)
}

pub async fn browser_insert_text(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::TextInputRequest>,
) -> WebResult<impl IntoResponse> {
    ops::insert_text(&state, &request).await.map(Json)
}

pub async fn browser_mode(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::ModeRequest>,
) -> WebResult<impl IntoResponse> {
    ops::set_mode(&state, &request).await.map(Json)
}

pub async fn browser_resize(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::ResizeRequest>,
) -> WebResult<impl IntoResponse> {
    ops::resize(&state, &request).await.map(Json)
}

pub async fn browser_close(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<ops::PaneRef>,
) -> WebResult<impl IntoResponse> {
    ops::close(&state, &request.pane_id).await.map(Json)
}

pub async fn browser_list(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    ops::list(&state).await.map(Json)
}
