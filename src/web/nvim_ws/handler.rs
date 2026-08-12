//! Authenticated WebSocket ownership and edit-engine lifecycle.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use axum::extract::ws::WebSocket;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};
use tracing::debug;

use super::egress::run as run_egress;
use super::ingress::run as run_ingress;
use crate::nvim_edit::EditEngine;
use crate::nvim_ui::stream::wire::ControlMsg;
use crate::nvim_ui::{NvimNotification, NvimSession, SessionKey, UiSize};
use crate::web::auth::check_auth_manual;
use crate::web::error::WebError;
use crate::web::types::ServerState;

#[derive(Debug, Deserialize)]
pub(crate) struct NvimUiQuery {
    #[serde(default)]
    pub project_idx: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
}

struct Owner {
    id: u64,
    supersede: mpsc::UnboundedSender<ControlMsg>,
}
struct Lease {
    key: SessionKey,
    id: u64,
    controls: mpsc::UnboundedSender<ControlMsg>,
    receiver: mpsc::UnboundedReceiver<ControlMsg>,
}
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
static OWNERS: OnceLock<Mutex<HashMap<SessionKey, Owner>>> = OnceLock::new();

fn owners() -> &'static Mutex<HashMap<SessionKey, Owner>> {
    OWNERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn claim(key: SessionKey) -> Lease {
    let (controls, receiver) = mpsc::unbounded_channel();
    let id = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    let previous = {
        let mut entries = owners()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.insert(
            key.clone(),
            Owner {
                id,
                supersede: controls.clone(),
            },
        )
    };
    if let Some(previous) = previous {
        let _ = previous.supersede.send(ControlMsg::Superseded {});
    }
    Lease {
        key,
        id,
        controls,
        receiver,
    }
}

#[cfg(test)]
fn release(lease: &Lease) {
    release_parts(&lease.key, lease.id);
}

fn release_parts(key: &SessionKey, id: u64) {
    let mut entries = owners()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if entries.get(key).is_some_and(|owner| owner.id == id) {
        entries.remove(key);
    }
}

pub(crate) async fn websocket_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Query(query): Query<NvimUiQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, WebError> {
    authorize(&state, &headers, &query.token)?;
    let session_id = query
        .session_id
        .filter(|id| !id.is_empty())
        .ok_or_else(|| WebError::BadRequest("session_id is required".into()))?;
    let project_idx = query
        .project_idx
        .unwrap_or(state.web_state.active_project_index().await);
    let project_dir = state
        .web_state
        .get_project_working_dir(project_idx)
        .await
        .ok_or_else(|| WebError::BadRequest("project does not exist".into()))?;
    let key = SessionKey::new(project_idx, session_id);
    let session = state
        .nvim_ui
        .ensure(key.clone(), &project_dir, UiSize::default())
        .await
        .map_err(|error| WebError::Internal(format!("failed to start Neovim: {error}")))?;
    let notifications = session.subscribe();
    let lease = claim(key);
    Ok(ws.on_upgrade(move |socket| run_connection(socket, session, notifications, lease)))
}

fn authorize(
    state: &ServerState,
    headers: &HeaderMap,
    token: &Option<String>,
) -> Result<(), WebError> {
    check_auth_manual(state, headers, token)
        .then_some(())
        .ok_or(WebError::Unauthorized)
}

async fn run_connection(
    socket: WebSocket,
    session: Arc<NvimSession>,
    notifications: broadcast::Receiver<NvimNotification>,
    lease: Lease,
) {
    let Lease {
        key,
        id,
        controls,
        receiver,
    } = lease;
    let (sender, socket_receiver) = socket.split();
    let _ = controls.send(ControlMsg::Ready {});
    let engine = EditEngine::new(session.clone(), controls.clone());
    let notification_task = tokio::spawn(engine.clone().notifications(notifications));
    let egress = run_egress(sender, receiver, session.clone());
    let ingress = run_ingress(socket_receiver, engine, controls);
    tokio::pin!(egress);
    tokio::pin!(ingress);
    tokio::select! { _ = &mut egress => {}, _ = &mut ingress => {} }
    notification_task.abort();
    release_parts(&key, id);
    debug!(key = ?session.key(), "Neovim edit-engine WebSocket ended");
}

#[cfg(test)]
#[path = "handler_tests.rs"]
mod handler_tests;
