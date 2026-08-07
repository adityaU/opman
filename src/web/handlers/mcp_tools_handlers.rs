//! `/api/mcp/servers/{name}/tools` — what a declared server actually exposes.
//!
//! The rest of the MCP section is an editor for `mcp.json`, which can only ever report
//! what the user wrote. This one asks the server, so the page can answer the question
//! configuration cannot: given this entry, which tools does an agent end up with, and on
//! what arguments.

use axum::extract::{Path, State};
use axum::Json;

use super::common::resolve_project_dir;
use crate::mcp_probe::Catalog;
use crate::web::auth::AuthUser;
use crate::web::error::{WebError, WebResult};
use crate::web::types::ServerState;

/// Launch one server, read its `tools/list`, and hand the outcome over whole.
///
/// A server that refuses to start is a 200 carrying that fact, not a 5xx: "declared but
/// broken" is a state the page renders, and a bare status code cannot say which server or
/// why. Only a name that is not a legal server name is rejected outright.
pub async fn list_tools(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<String>,
) -> WebResult<Json<Catalog>> {
    let name = validate_name(&name)?;
    let dir = resolve_project_dir(&state).await?;
    let registry = state.mcp.current();
    Ok(Json(crate::mcp_probe::catalog(&registry, &name, &dir).await))
}

/// The same name rule the editing endpoints enforce, reported as a message rather than a
/// bare status — this one reaches a panel that has room to say what was wrong.
fn validate_name(raw: &str) -> WebResult<String> {
    super::mcp_handlers::validate(raw).map_err(|_| {
        WebError::BadRequest(
            "An MCP server name is 1–64 characters of letters, digits, dot, dash or \
             underscore."
                .into(),
        )
    })
}

#[cfg(test)]
#[path = "mcp_tools_handlers_tests.rs"]
mod mcp_tools_handlers_tests;
