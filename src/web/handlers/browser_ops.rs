//! The browser pane operations, independent of who is asking.
//!
//! Both the authenticated `/api/browser/*` routes the pane uses and the loopback
//! `/internal/browser/*` routes the MCP server uses funnel through here, so the agent and
//! the human genuinely drive the same page by the same code — the shared-pane promise is
//! structural rather than a convention two handlers have to keep.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::browser::{MouseKind, Opened, RenderMode, SnapshotOptions};

use super::super::error::{WebError, WebResult};
use super::super::types::ServerState;

/// Every request names a pane; the pane is the session.
#[derive(Debug, Deserialize)]
pub struct PaneRef {
    pub pane_id: String,
}

#[derive(Debug, Deserialize)]
pub struct NavigateRequest {
    pub pane_id: String,
    /// The project the browser belongs to. Only consulted when the tab has to be created;
    /// an existing tab keeps the project it was opened for.
    #[serde(default)]
    pub project: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotQuery {
    pub pane_id: String,
    #[serde(flatten)]
    pub options: SnapshotOptions,
}

#[derive(Debug, Deserialize)]
pub struct TextQuery {
    pub pane_id: String,
    pub max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ScreenshotQuery {
    pub pane_id: String,
    pub quality: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct ClickRequest {
    pub pane_id: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Deserialize)]
pub struct TypeRequest {
    pub pane_id: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub text: String,
    #[serde(default)]
    pub submit: bool,
}

#[derive(Debug, Deserialize)]
pub struct KeyRequest {
    pub pane_id: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct ScrollRequest {
    pub pane_id: String,
    #[serde(default)]
    pub x: i64,
    #[serde(default)]
    pub y: i64,
    pub delta_y: i64,
}

/// Raw pointer phases, forwarded from a screencast pane so a drag or a hover reads the
/// same to the page as it would in a real window.
#[derive(Debug, Deserialize)]
pub struct MouseRequest {
    pub pane_id: String,
    pub kind: MouseKind,
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Deserialize)]
pub struct TextInputRequest {
    pub pane_id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ModeRequest {
    pub pane_id: String,
    pub mode: RenderMode,
}

#[derive(Debug, Deserialize)]
pub struct ResizeRequest {
    pub pane_id: String,
    pub width: u32,
    pub height: u32,
}

/// CDP failures are the caller's problem (a bad ref, an unreachable host), not a server
/// fault — surface the message rather than a bare 500.
fn bad_request(error: anyhow::Error) -> WebError {
    WebError::BadRequest(error.to_string())
}

/// Resolve an already-open pane. Acting on a pane that was never opened is a mistake
/// worth reporting, not a reason to silently spawn a tab.
async fn require(state: &ServerState, pane_id: &str) -> WebResult<std::sync::Arc<crate::browser::Pane>> {
    state
        .browser
        .get(pane_id)
        .await
        .ok_or(WebError::NotFound("browser pane is not open"))
}

/// Connect a pane to its browser, sending it to `url` only if the tab is new.
///
/// This is what makes reopening a browser widget *reconnect* rather than reset: the tab
/// lives in the server and may well have been driven somewhere else by an agent since the
/// widget was last on screen, and that page — not the widget's saved one — is the truth.
/// An explicit navigation is a separate call.
pub async fn open(
    state: &ServerState,
    pane_id: &str,
    project: &str,
    url: Option<&str>,
) -> WebResult<Value> {
    let (pane, opened) = state
        .browser
        .open(pane_id, project)
        .await
        .map_err(bad_request)?;

    let live = pane.current_url().await;
    let adopting = opened == Opened::Adopted && live != crate::browser::BLANK;
    if adopting {
        return Ok(json!({
            "paneId": pane_id,
            "project": project,
            "mode": pane.mode().await,
            "url": live,
            "title": pane.tab().title().await.unwrap_or_default(),
            "adopted": true,
        }));
    }

    match url {
        Some(url) => navigate(state, pane_id, project, url).await,
        None => Ok(json!({
            "paneId": pane_id,
            "project": project,
            "mode": RenderMode::Screencast,
            "url": crate::browser::BLANK,
            "title": "",
            "adopted": false,
        })),
    }
}

pub async fn navigate(
    state: &ServerState,
    pane_id: &str,
    project: &str,
    url: &str,
) -> WebResult<Value> {
    let mode = state
        .browser
        .navigate(pane_id, project, url)
        .await
        .map_err(bad_request)?;
    let pane = require(state, pane_id).await?;
    let page = pane
        .tab()
        .snapshot(SnapshotOptions::default())
        .await
        .map_err(bad_request)?;
    Ok(json!({
        "paneId": pane_id,
        "project": project,
        "mode": mode,
        "url": page.url,
        "title": page.title,
        "adopted": false,
    }))
}

/// `back`, `forward`, and `reload` share a shape; keeping them one function is what keeps
/// the route table from growing three near-identical handlers.
pub enum Step {
    Back,
    Forward,
    Reload,
}

pub async fn step(state: &ServerState, pane_id: &str, step: Step) -> WebResult<Value> {
    let pane = require(state, pane_id).await?;
    let tab = pane.tab();
    match step {
        Step::Back => tab.go_back().await,
        Step::Forward => tab.go_forward().await,
        Step::Reload => tab.reload().await,
    }
    .map_err(bad_request)?;

    let page = tab
        .snapshot(SnapshotOptions::default())
        .await
        .map_err(bad_request)?;
    Ok(json!({ "paneId": pane_id, "url": page.url, "title": page.title }))
}

pub async fn snapshot(
    state: &ServerState,
    pane_id: &str,
    options: SnapshotOptions,
) -> WebResult<Value> {
    let pane = require(state, pane_id).await?;
    let page = pane.tab().snapshot(options).await.map_err(bad_request)?;
    serde_json::to_value(page).map_err(|e| WebError::Internal(e.to_string()))
}

pub async fn read_text(
    state: &ServerState,
    pane_id: &str,
    max_chars: Option<usize>,
) -> WebResult<Value> {
    let pane = require(state, pane_id).await?;
    let text = pane
        .tab()
        .read_text(max_chars)
        .await
        .map_err(bad_request)?;
    serde_json::to_value(text).map_err(|e| WebError::Internal(e.to_string()))
}

pub async fn screenshot(state: &ServerState, pane_id: &str, quality: Option<u8>) -> WebResult<Value> {
    let pane = require(state, pane_id).await?;
    let data = pane
        .tab()
        .screenshot(quality.unwrap_or(60))
        .await
        .map_err(bad_request)?;
    Ok(json!({ "paneId": pane_id, "mimeType": "image/jpeg", "data": data }))
}

/// Act, then hand back a fresh outline. One round trip instead of two is worth real
/// tokens: the model would ask "what happened?" after every click anyway.
pub async fn click(state: &ServerState, request: &ClickRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .click_ref(&request.reference)
        .await
        .map_err(bad_request)?;
    after_action(&pane, &request.pane_id).await
}

pub async fn type_text(state: &ServerState, request: &TypeRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .type_ref(&request.reference, &request.text, request.submit)
        .await
        .map_err(bad_request)?;
    after_action(&pane, &request.pane_id).await
}

pub async fn press_key(state: &ServerState, request: &KeyRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .press_key(&request.key)
        .await
        .map_err(bad_request)?;
    after_action(&pane, &request.pane_id).await
}

pub async fn scroll(state: &ServerState, request: &ScrollRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .scroll(request.x, request.y, request.delta_y)
        .await
        .map_err(bad_request)?;
    Ok(json!({ "ok": true }))
}

/// Pane-originated pointer phases. No snapshot afterwards — a human moving the mouse does
/// not need the page re-described, and a move event fires dozens of times a second.
pub async fn mouse(state: &ServerState, request: &MouseRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .mouse(request.kind, request.x, request.y)
        .await
        .map_err(bad_request)?;
    Ok(json!({ "ok": true }))
}

pub async fn insert_text(state: &ServerState, request: &TextInputRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .insert_text(&request.text)
        .await
        .map_err(bad_request)?;
    Ok(json!({ "ok": true }))
}

pub async fn set_mode(state: &ServerState, request: &ModeRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.set_mode(request.mode).await;
    Ok(json!({ "paneId": request.pane_id, "mode": request.mode }))
}

pub async fn resize(state: &ServerState, request: &ResizeRequest) -> WebResult<Value> {
    let pane = require(state, &request.pane_id).await?;
    pane.tab()
        .resize(request.width.clamp(200, 3840), request.height.clamp(200, 2160))
        .await
        .map_err(bad_request)?;
    Ok(json!({ "ok": true }))
}

pub async fn close(state: &ServerState, pane_id: &str) -> WebResult<Value> {
    state.browser.close(pane_id).await;
    Ok(json!({ "ok": true }))
}

/// The pane id of a project's browser, opening one if the project has none.
///
/// Agents name a project, not a pane: the pane id is a workspace detail they have no way
/// to know, and "the browser for this repo" is the thing they actually mean. The id
/// generated here matches the one the widget derives from the same project path, so a
/// browser an agent opens is the browser the user's pane connects to.
pub async fn pane_for_project(state: &ServerState, project: &str) -> WebResult<String> {
    if let Some((pane_id, _)) = state.browser.for_project(project).await {
        return Ok(pane_id.to_string());
    }
    let pane_id = crate::browser::pane_id_for_project(project);
    state
        .browser
        .open(&pane_id, project)
        .await
        .map_err(bad_request)?;
    Ok(pane_id)
}

pub async fn list(state: &ServerState) -> WebResult<Value> {
    let panes = state.browser.list().await;
    serde_json::to_value(json!({ "panes": panes })).map_err(|e| WebError::Internal(e.to_string()))
}

/// The outline that follows an action, plus where the page ended up.
async fn after_action(pane: &crate::browser::Pane, pane_id: &str) -> WebResult<Value> {
    let page = pane
        .tab()
        .snapshot(SnapshotOptions::default())
        .await
        .map_err(bad_request)?;
    let mut value = serde_json::to_value(page).map_err(|e| WebError::Internal(e.to_string()))?;
    if let Some(object) = value.as_object_mut() {
        object.insert("paneId".into(), json!(pane_id));
    }
    Ok(value)
}
