//! Agent listing handler with upstream proxy + config fallback.

use axum::extract::State;
use axum::response::{IntoResponse, Json};

use super::super::auth::AuthUser;
use super::super::error::WebResult;
use super::super::types::*;
use super::common::resolve_project_dir;
use crate::app::base_url;

/// GET /api/agents — list available agents.
///
/// Primary path: proxies `GET {opencode}/agent` to get the live, fully-resolved
/// agent list from the running opencode instance (same as opencode's own web UI).
///
/// Fallback: if the opencode instance is unreachable, reads the project's
/// `opencode.json` / `.opencode/config.json` for agent definitions and injects
/// built-in defaults.
pub async fn get_agents(
    State(state): State<ServerState>,
    _auth: AuthUser,
) -> WebResult<impl IntoResponse> {
    let dir = resolve_project_dir(&state).await?;
    let base = base_url().to_string();

    // ── Primary: query the running opencode instance ────────────────
    if let Ok(resp) = state.http_client
        .get(format!("{}/agent", base))
        .header("x-opencode-directory", &dir)
        .header("Accept", "application/json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(upstream) = resp.json::<Vec<serde_json::Value>>().await {
                let agents: Vec<AgentEntry> = upstream
                    .iter()
                    .map(|v| AgentEntry {
                        id: v.get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string(),
                        label: {
                            let name = v.get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("");
                            // Capitalize first letter for display
                            let mut chars = name.chars();
                            match chars.next() {
                                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                                None => name.to_string(),
                            }
                        },
                        description: v.get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string(),
                        mode: v.get("mode")
                            .and_then(|m| m.as_str())
                            .unwrap_or("all")
                            .to_string(),
                        hidden: v.get("hidden")
                            .and_then(|h| h.as_bool())
                            .unwrap_or(false),
                        native: v.get("native")
                            .and_then(|n| n.as_bool())
                            .unwrap_or(false),
                        color: v.get("color")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string()),
                    })
                    .collect();

                if !agents.is_empty() {
                    return Ok(Json(agents));
                }
            }
        }
    }

    // ── Fallback: read static config files ──────────────────────────
    let dir_path = std::path::Path::new(&dir);
    let config_paths = [
        dir_path.join("opencode.json"),
        dir_path.join(".opencode/config.json"),
        dir_path.join(".opencode.json"),
    ];

    let mut agents = Vec::new();

    for config_path in &config_paths {
        if let Ok(content) = tokio::fs::read_to_string(config_path).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(agents_obj) = json.get("agents").and_then(|a| a.as_object()) {
                    for (id, agent_config) in agents_obj {
                        let description = agent_config
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string();
                        let label = agent_config
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| {
                                let mut chars = id.chars();
                                match chars.next() {
                                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                                    None => id.clone(),
                                }
                            });
                        let mode = agent_config
                            .get("mode")
                            .and_then(|m| m.as_str())
                            .unwrap_or("all")
                            .to_string();
                        let hidden = agent_config
                            .get("hidden")
                            .and_then(|h| h.as_bool())
                            .unwrap_or(false);
                        let color = agent_config
                            .get("color")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string());
                        agents.push(AgentEntry {
                            id: id.clone(),
                            label,
                            description,
                            mode,
                            hidden,
                            native: false,
                            color,
                        });
                    }
                }
                break;
            }
        }
    }

    // Ensure built-in defaults (must match upstream opencode agent names)
    let has_build = agents.iter().any(|a| a.id == "build");
    let has_plan = agents.iter().any(|a| a.id == "plan");

    if !has_build {
        agents.insert(0, AgentEntry {
            id: "build".to_string(),
            label: "Build".to_string(),
            description: "Default coding agent".to_string(),
            mode: "primary".to_string(),
            hidden: false,
            native: true,
            color: None,
        });
    }
    if !has_plan {
        agents.push(AgentEntry {
            id: "plan".to_string(),
            label: "Plan".to_string(),
            description: "Planning and design agent".to_string(),
            mode: "all".to_string(),
            hidden: false,
            native: true,
            color: None,
        });
    }

    for agent in &mut agents {
        if agent.id == "build" || agent.id == "plan" {
            agent.native = true;
        }
    }

    agents.sort_by(|a, b| {
        let order = |id: &str| -> usize {
            match id {
                "build" => 0,
                "plan" => 1,
                _ => 2,
            }
        };
        order(&a.id).cmp(&order(&b.id)).then_with(|| a.id.cmp(&b.id))
    });

    Ok(Json(agents))
}
