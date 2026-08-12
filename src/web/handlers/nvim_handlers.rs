use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use std::str::FromStr;

use super::super::auth::AuthUser;
use super::super::error::{WebError, WebResult};
use super::super::types::ServerState;
use super::common::resolve_editor_nvim_socket;
use crate::mcp::{Capability, NvimOp, SocketRequest, SocketResponse};

fn forbidden(message: &'static str) -> WebError {
    WebError::Upstream(StatusCode::FORBIDDEN, message.into())
}

pub async fn proxy_nvim(
    State(state): State<ServerState>,
    _auth: AuthUser,
    Json(request): Json<SocketRequest>,
) -> WebResult<Json<SocketResponse>> {
    let op = NvimOp::from_str(&request.op)
        .map_err(|_| WebError::BadRequest(format!("Unknown nvim operation: {}", request.op)))?;

    let browser_command = match op.capability() {
        Capability::Execute if op == NvimOp::Command => {
            let command = request.command.as_deref().ok_or(forbidden(
                "Browser nvim_command requires one of the allowed editor commands",
            ))?;
            Some(
                crate::mcp::nvim_handler::parse_browser_command(command).map_err(|_| {
                    forbidden("Browser nvim_command is restricted to the editor command set")
                })?,
            )
        }
        Capability::Execute => {
            return Err(forbidden(
                "Browser Neovim execution operations are not permitted",
            ));
        }
        Capability::Read | Capability::Edit => None,
    };

    let session_id = request
        .session_id
        .as_deref()
        .ok_or(WebError::BadRequest("session_id is required".into()))?;
    let socket = resolve_editor_nvim_socket(&state, session_id).await?;
    let result = tokio::task::spawn_blocking(move || match browser_command {
        Some(command) => crate::mcp::nvim_handler::handle_browser_nvim_command(&socket, command),
        None => crate::mcp::nvim_handler::handle_nvim_op_blocking(&socket, op, &request),
    })
    .await
    .map_err(|error| WebError::Internal(format!("Neovim proxy task failed: {error}")))?;
    Ok(Json(result))
}

#[cfg(test)]
#[path = "nvim_handlers_tests.rs"]
mod nvim_handlers_tests;
