//! PTY API helpers — spawn, write, resize, kill, list.
//! Matches React `api/pty.ts`.

use serde::Serialize;

use super::client::{api_fetch, api_post, api_post_void, ApiError};
use crate::types::api::{PtyListResponse, SpawnPtyResponse};

#[derive(Serialize)]
struct SpawnBody<'a> {
    kind: &'a str,
    id: &'a str,
    rows: u16,
    cols: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
}

/// Spawn a new PTY session (matches backend SpawnPtyRequest).
pub async fn pty_spawn(
    kind: &str,
    id: &str,
    rows: u16,
    cols: u16,
    session_id: Option<&str>,
) -> Result<SpawnPtyResponse, ApiError> {
    api_post("/pty/spawn", &SpawnBody { kind, id, rows, cols, session_id }).await
}

#[derive(Serialize)]
struct WriteBody<'a> {
    id: &'a str,
    data: &'a str,
}

/// Write data (base64-encoded) to a PTY.
pub async fn pty_write(id: &str, data: &str) -> Result<(), ApiError> {
    api_post_void("/pty/write", &WriteBody { id, data }).await
}

#[derive(Serialize)]
struct ResizeBody<'a> {
    id: &'a str,
    rows: u16,
    cols: u16,
}

/// Resize a PTY.
pub async fn pty_resize(id: &str, rows: u16, cols: u16) -> Result<(), ApiError> {
    api_post_void("/pty/resize", &ResizeBody { id, rows, cols }).await
}

#[derive(Serialize)]
struct KillBody<'a> {
    id: &'a str,
}

/// Kill a PTY.
pub async fn pty_kill(id: &str) -> Result<(), ApiError> {
    api_post_void("/pty/kill", &KillBody { id }).await
}

/// List active PTYs.
pub async fn pty_list() -> Result<PtyListResponse, ApiError> {
    api_fetch("/pty/list").await
}
