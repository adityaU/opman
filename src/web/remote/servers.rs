//! `GET/POST /api/servers`, `PATCH/DELETE /api/servers/{id}`. Passwords go in, never out.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use super::{settings_error, WebError};
use crate::remote::{NewServer, ServerId, ServerPatch, ServerSnapshot, ServerStatus, HOME_ID};
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    id: String,
    name: String,
    url: Option<String>,
    base: String,
    status: ServerStatus,
}

impl ServerInfo {
    fn home(state: &ServerState) -> Self {
        Self {
            id: HOME_ID.to_owned(),
            name: state.instance_name.as_deref().unwrap_or("Home").to_owned(),
            url: None,
            base: String::new(),
            status: ServerStatus::Ok,
        }
    }

    fn remote(snapshot: ServerSnapshot, status: ServerStatus) -> Self {
        Self {
            base: format!("/remote/{}", snapshot.id),
            id: snapshot.id.to_string(),
            name: snapshot.name,
            url: Some(snapshot.url.as_str().to_owned()),
            status,
        }
    }
}

#[derive(Deserialize)]
pub struct AddServer {
    name: String,
    url: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateServer {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

pub async fn list(_auth: AuthUser, State(state): State<ServerState>) -> Json<Vec<ServerInfo>> {
    let remotes = state.remote.statuses().await;
    let mut out = Vec::with_capacity(remotes.len() + 1);
    out.push(ServerInfo::home(&state));
    out.extend(
        remotes
            .into_iter()
            .map(|(snapshot, status)| ServerInfo::remote(snapshot, status)),
    );
    Json(out)
}

pub async fn add(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Json(body): Json<AddServer>,
) -> Result<Json<ServerInfo>, WebError> {
    let new = NewServer {
        name: body.name,
        url: body.url,
        username: body.username,
        password: body.password,
    };
    let snapshot = state.remote.add(new).await.map_err(settings_error)?;
    Ok(Json(ServerInfo::remote(snapshot, ServerStatus::Ok)))
}

pub async fn update(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateServer>,
) -> Result<Json<ServerInfo>, WebError> {
    let id = ServerId::parse(&id).ok_or(WebError::NotFound("unknown server"))?;
    let patch = ServerPatch {
        name: body.name,
        url: body.url,
        username: body.username,
        password: body.password,
    };
    let (snapshot, status) = state
        .remote
        .update(&id, patch)
        .await
        .map_err(settings_error)?;
    Ok(Json(ServerInfo::remote(snapshot, status)))
}

pub async fn remove(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<StatusCode, WebError> {
    let id = ServerId::parse(&id).ok_or(WebError::NotFound("unknown server"))?;
    state.remote.remove(&id).await.map_err(settings_error)?;
    Ok(StatusCode::NO_CONTENT)
}
