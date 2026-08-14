//! Loopback-only browser API, called by the `opman mcp-browser` server.
//!
//! Mounted outside `/api`, so it carries the shared `X-Internal-Token` instead of the
//! browser's `AuthUser`. One route rather than seventeen: the operation is a tagged enum
//! in the body, which keeps the MCP client to a single request shape and makes an unknown
//! operation a deserialisation error rather than a 404 to interpret.
//!
//! Agents address a **project**, never a pane id. A pane id is a workspace detail an agent
//! has no way to learn, whereas "the browser for the repo I am working in" is what it
//! actually means — and resolving it here is what lets the agent and the user's pane meet
//! on the same tab without either being told about the other.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Json};
use serde::Deserialize;

use crate::browser::SnapshotOptions;

use super::super::error::{WebError, WebResult};
use super::super::types::{ServerState, WebEvent};
use super::browser_ops::{self as ops, Step};

/// What the MCP server wants done. `project` rides alongside on the envelope rather than
/// inside each arm, because every operation needs it and nothing else does.
#[derive(Debug, Deserialize)]
pub struct BrowserCall {
    /// Absolute path of the agent's project. Empty means "whichever browser is open",
    /// which is what a single-project workspace should not have to spell out.
    #[serde(default)]
    pub project: String,
    #[serde(flatten)]
    pub operation: Operation,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Open {
        url: Option<String>,
    },
    Navigate {
        url: String,
    },
    Back,
    Forward,
    Reload,
    Snapshot {
        #[serde(flatten)]
        options: SnapshotOptions,
    },
    Text {
        max_chars: Option<usize>,
    },
    Screenshot {
        quality: Option<u8>,
    },
    Click {
        #[serde(rename = "ref")]
        reference: String,
    },
    Type {
        #[serde(rename = "ref")]
        reference: String,
        text: String,
        #[serde(default)]
        submit: bool,
    },
    Key {
        key: String,
    },
    Scroll {
        delta_y: i64,
    },
    Close,
    List,
}

impl Operation {
    /// Whether this operation can move the page. Only those are worth telling the UI
    /// about — a snapshot changes nothing a pane would want to react to.
    const fn navigates(&self) -> bool {
        matches!(
            self,
            Self::Open { .. } | Self::Navigate { .. } | Self::Back | Self::Forward | Self::Click { .. } | Self::Type { .. }
        )
    }
}

fn check_internal_token(state: &ServerState, headers: &HeaderMap) -> WebResult<()> {
    let provided = headers
        .get("x-internal-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !state.internal_token.is_empty() && provided == state.internal_token {
        return Ok(());
    }
    Err(WebError::Unauthorized)
}

/// POST /internal/browser
pub async fn internal_browser(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(call): Json<BrowserCall>,
) -> WebResult<impl IntoResponse> {
    check_internal_token(&state, &headers)?;

    // Listing spans every project, so it must not first conjure a browser for this one.
    if matches!(call.operation, Operation::List) {
        return ops::list(&state).await.map(Json);
    }

    let project = resolve_project(&state, &call.project).await?;
    let pane_id = ops::pane_for_project(&state, &project).await?;
    let navigates = call.operation.navigates();

    let value = match call.operation {
        Operation::Open { url } => ops::open(&state, &pane_id, &project, url.as_deref()).await,
        Operation::Navigate { url } => ops::navigate(&state, &pane_id, &project, &url).await,
        Operation::Back => ops::step(&state, &pane_id, Step::Back).await,
        Operation::Forward => ops::step(&state, &pane_id, Step::Forward).await,
        Operation::Reload => ops::step(&state, &pane_id, Step::Reload).await,
        Operation::Snapshot { options } => ops::snapshot(&state, &pane_id, options).await,
        Operation::Text { max_chars } => ops::read_text(&state, &pane_id, max_chars).await,
        Operation::Screenshot { quality } => ops::screenshot(&state, &pane_id, quality).await,
        Operation::Click { reference } => {
            ops::click(&state, &pane_ref(&pane_id, reference)).await
        }
        Operation::Type {
            reference,
            text,
            submit,
        } => {
            let request = ops::TypeRequest {
                pane_id: pane_id.clone(),
                reference,
                text,
                submit,
            };
            ops::type_text(&state, &request).await
        }
        Operation::Key { key } => {
            let request = ops::KeyRequest {
                pane_id: pane_id.clone(),
                key,
            };
            ops::press_key(&state, &request).await
        }
        Operation::Scroll { delta_y } => {
            let request = ops::ScrollRequest {
                pane_id: pane_id.clone(),
                x: 0,
                y: 0,
                delta_y,
            };
            ops::scroll(&state, &request).await
        }
        Operation::Close => ops::close(&state, &pane_id).await,
        Operation::List => unreachable!("listing returned above"),
    }?;

    // Tell the workspace where the agent went, so the pane showing that project's browser
    // comes to the front — or gets created — instead of the user finding out later.
    if navigates {
        let url = value
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let _ = state.event_tx.send(WebEvent::McpBrowserOpen {
            project_path: project,
            browser_id: pane_id,
            url,
        });
    }

    Ok(Json(value))
}

/// The project to act on: the one named, or the only sensible default.
///
/// An agent that names nothing gets the browser that is already open; failing that, the
/// active project. Guessing is better than refusing here — the common case is one project,
/// where naming it is pure ceremony.
async fn resolve_project(state: &ServerState, named: &str) -> WebResult<String> {
    if !named.is_empty() {
        return Ok(named.to_string());
    }
    if let Some(pane) = state.browser.list().await.first() {
        return Ok(pane.project.to_string());
    }
    state
        .web_state
        .get_working_dir()
        .await
        .map(|dir| dir.to_string_lossy().into_owned())
        .ok_or(WebError::BadRequest(
            "no project to open a browser for — pass `project`".into(),
        ))
}

fn pane_ref(pane_id: &str, reference: String) -> ops::ClickRequest {
    ops::ClickRequest {
        pane_id: pane_id.to_string(),
        reference,
    }
}

#[cfg(test)]
#[path = "browser_internal_tests.rs"]
mod browser_internal_tests;
