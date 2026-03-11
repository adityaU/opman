//! Shared helper functions used across handler modules.

use std::path::PathBuf;

use super::super::error::{WebError, WebResult};
use super::super::types::*;

/// Constant-time byte comparison to prevent timing side-channel attacks.
pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub(super) async fn resolve_editor_nvim_socket(
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

pub(super) async fn resolve_editor_buffer(
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

/// Helper: resolve project directory from web state.
pub(super) async fn resolve_project_dir(state: &ServerState) -> WebResult<String> {
    state
        .web_state
        .get_working_dir()
        .await
        .map(|p| p.to_string_lossy().to_string())
        .ok_or(WebError::BadRequest("No active project".into()))
}
