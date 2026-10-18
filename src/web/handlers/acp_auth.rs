//! `GET /api/acp/agents/{id}/auth`: is this agent logged in, and how would it be.
//!
//! The answer comes from asking the agent: a short-lived copy is started, asked for a session,
//! and stopped (see `acp_engine::diagnose`). A session means connected; "authentication
//! required" means not. Nothing is logged in from here, because a login can need a terminal.
//! The page opens one running the `loginCommand` returned below.

use std::time::Duration;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::acp_engine::config;
use crate::acp_engine::diagnose::{self, Probe, Session};
use crate::web::auth::AuthUser;

/// How long the agent gets for each of `initialize` and `session/new`.
const STEP_LIMIT: Duration = Duration::from_secs(20);

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Connected,
    NotConnected,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthView {
    pub state: AuthState,
    /// Why it is not connected, in the agent's own words where it gave any. Empty when it is.
    pub detail: String,
    /// The login methods the agent offers, by display name.
    pub methods: Vec<String>,
    /// A shell command that logs the agent in. Present only when a login is possible.
    pub login_command: Option<String>,
}

pub async fn agent_auth(
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AuthView>, StatusCode> {
    let id = super::acp_handlers::validate_id(&id)?;
    let document = config::load();
    let agent = document.agents.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if !agent.launchable() {
        return Ok(Json(not_connected("Turn the agent on first.")));
    }
    let probe = diagnose::probe(&id, agent, STEP_LIMIT).await;
    let exe = std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "opman".to_string());
    Ok(Json(view_of(&id, &exe, probe)))
}

fn not_connected(detail: &str) -> AuthView {
    AuthView {
        state: AuthState::NotConnected,
        detail: detail.to_string(),
        methods: Vec::new(),
        login_command: None,
    }
}

/// What a probe means for the page. `exe` is how to run this opman from a shell.
fn view_of(id: &str, exe: &str, probe: Result<Probe, String>) -> AuthView {
    let probe = match probe {
        Ok(probe) => probe,
        Err(reason) => return not_connected(&reason),
    };
    let methods = probe.hello.auth_methods;
    let detail = match probe.session {
        Session::Offered(_) => {
            return AuthView {
                state: AuthState::Connected,
                detail: String::new(),
                methods,
                login_command: None,
            }
        }
        Session::LoginRequired => "Log in to use this agent.".to_string(),
        Session::Failed(reason) => reason,
    };
    let login_command = (!methods.is_empty()).then(|| format!("{} acp-login {id}", quoted(exe)));
    AuthView {
        state: AuthState::NotConnected,
        detail,
        methods,
        login_command,
    }
}

/// Single-quoted for a POSIX shell, so a path with spaces or quotes stays one word.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
#[path = "acp_auth_tests.rs"]
mod acp_auth_tests;
