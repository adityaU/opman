//! Session management API — matches React `api/session.ts`.

use serde::Serialize;
use crate::types::api::{AgentInfo, ThemePreview};
use super::client::{api_fetch, api_post, api_delete, api_patch, api_put, ApiError};

// ── Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub context_length: u64,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default)]
    pub supports_streaming: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub models: Vec<ProviderModel>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandDef {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub args: Vec<CommandArg>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandArg {
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommandsResponse {
    pub commands: Vec<CommandDef>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AgentsResponse {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemesResponse {
    pub themes: Vec<ThemePreview>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TodosResponse {
    pub todos: Vec<TodoItem>,
}

// ── API functions ───────────────────────────────────────────────────

/// Delete a session.
pub async fn delete_session(session_id: &str) -> Result<(), ApiError> {
    let path = format!(
        "/session/{}",
        js_sys::encode_uri_component(session_id),
    );
    api_delete(&path).await
}

/// Rename a session.
pub async fn rename_session(session_id: &str, title: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        title: &'a str,
    }
    let path = format!(
        "/session/{}",
        js_sys::encode_uri_component(session_id),
    );
    let _: serde_json::Value = api_patch(&path, &Body { title }).await?;
    Ok(())
}

/// Execute a slash command in a session.
pub async fn execute_command(
    session_id: &str,
    command: &str,
    args: Option<&str>,
    model: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        command: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<&'a str>,
    }
    let path = format!(
        "/session/{}/command",
        js_sys::encode_uri_component(session_id),
    );
    api_post(&path, &Body { command, args, model }).await
}

/// Fetch available slash commands.
pub async fn fetch_commands() -> Result<Vec<CommandDef>, ApiError> {
    let resp: CommandsResponse = api_fetch("/commands").await?;
    Ok(resp.commands)
}

/// Fetch LLM providers and models.
pub async fn fetch_providers() -> Result<ProvidersResponse, ApiError> {
    api_fetch("/providers").await
}

/// Fetch available agents.
pub async fn fetch_agents() -> Result<Vec<AgentInfo>, ApiError> {
    let resp: AgentsResponse = api_fetch("/agents").await?;
    Ok(resp.agents)
}

/// Fetch todos for a session.
pub async fn fetch_session_todos(session_id: &str) -> Result<Vec<TodoItem>, ApiError> {
    let path = format!(
        "/session/{}/todos",
        js_sys::encode_uri_component(session_id),
    );
    let resp: TodosResponse = api_fetch(&path).await?;
    Ok(resp.todos)
}

/// Update todos for a session.
pub async fn update_session_todos(session_id: &str, todos: &[TodoItem]) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        todos: &'a [TodoItem],
    }
    let path = format!(
        "/session/{}/todos",
        js_sys::encode_uri_component(session_id),
    );
    let _: serde_json::Value = api_put(&path, &Body { todos }).await?;
    Ok(())
}

/// Fetch all available themes.
pub async fn fetch_themes() -> Result<Vec<ThemePreview>, ApiError> {
    let resp: ThemesResponse = api_fetch("/themes").await?;
    Ok(resp.themes)
}

/// Switch active theme.
pub async fn switch_theme(name: &str) -> Result<(), ApiError> {
    #[derive(Serialize)]
    struct Body<'a> {
        name: &'a str,
    }
    super::client::api_post_void("/theme/switch", &Body { name }).await
}
