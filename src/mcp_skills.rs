use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use axum::{extract::State, http::StatusCode, Json};
use anyhow::Result;
use crate::web::types::ServerState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Value,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
    id: Value,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    code: i32,
    message: String,
}

pub type SkillsRegistry = Arc<RwLock<HashMap<String, Skill>>>;

pub fn get_skills_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("opman")
        .join("skills")
}

pub async fn load_skills() -> Result<HashMap<String, Skill>> {
    let skills_dir = get_skills_dir();
    let mut skills = HashMap::new();

    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir)?;
        return Ok(skills);
    }

    for entry in std::fs::read_dir(&skills_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                if let Ok(skill) = parse_skill(&skill_md) {
                    skills.insert(skill.name.clone(), skill);
                }
            }
        }
    }

    Ok(skills)
}

fn parse_skill(path: &PathBuf) -> Result<Skill> {
    let content = std::fs::read_to_string(path)?;
    let parts: Vec<&str> = content.split("---").collect();
    if parts.len() < 3 {
        return Err(anyhow::anyhow!("Invalid SKILL.md format"));
    }
    let frontmatter: Value = serde_yaml::from_str(parts[1])?;
    let name = frontmatter.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let description = frontmatter.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let content = parts[2..].join("---").trim().to_string();
    Ok(Skill { name, description, content })
}

pub fn spawn_mcp_skills_server(reload_rx: broadcast::Receiver<()>, registry: SkillsRegistry) {
    let registry_clone = registry.clone();
    tokio::spawn(async move {
        let mut rx = reload_rx;
        loop {
            if rx.recv().await.is_ok() {
                if let Ok(new_skills) = load_skills().await {
                    *registry_clone.write().await = new_skills;
                    tracing::info!("Reloaded skills registry");
                }
            }
        }
    });
}

pub async fn mcp_handler(
    State(state): State<ServerState>,
    Json(req): Json<McpRequest>,
) -> Result<Json<McpResponse>, StatusCode> {
    let registry = &state.skills_registry;
    let resp = match req.method.as_str() {
        "initialize" => McpResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "opman-skills", "version": "1.0.0" }
            })),
            error: None,
            id: req.id,
        },
        "tools/list" => {
            let _skills = registry.read().await;
            let tools: Vec<Value> = vec![
                serde_json::json!({
                    "name": "list_skills",
                    "description": "List all available skills with names and descriptions",
                    "inputSchema": { "type": "object", "properties": {} }
                }),
                serde_json::json!({
                    "name": "load_skill",
                    "description": "Load the full content of a specific skill by name",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "The name of the skill to load" }
                        },
                        "required": ["name"]
                    }
                }),
            ];
            McpResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({ "tools": tools })),
                error: None,
                id: req.id,
            }
        }
        "tools/call" => {
            let params = req.params.unwrap_or(Value::Null);
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            let content = dispatch_tool(&registry, tool_name, &args).await;
            McpResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::json!({ "content": content })),
                error: None,
                id: req.id,
            }
        }
        _ => McpResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(McpError { code: -32601, message: "Method not found".to_string() }),
            id: req.id,
        },
    };
    Ok(Json(resp))
}

async fn dispatch_tool(registry: &SkillsRegistry, tool_name: &str, args: &Value) -> Value {
    match tool_name {
        "list_skills" => {
            let skills = registry.read().await;
            let list: Vec<Value> = skills.values().map(|s| serde_json::json!({
                "name": s.name,
                "description": s.description
            })).collect();
            serde_json::json!([{ "type": "text", "text": serde_json::to_string(&list).unwrap() }])
        }
        "load_skill" => {
            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                let skills = registry.read().await;
                if let Some(skill) = skills.get(name) {
                    serde_json::json!([{ "type": "text", "text": skill.content.clone() }])
                } else {
                    serde_json::json!([{ "type": "text", "text": format!("Skill '{}' not found", name) }])
                }
            } else {
                serde_json::json!([{ "type": "text", "text": "Missing 'name' argument" }])
            }
        }
        _ => serde_json::json!([{ "type": "text", "text": format!("Unknown tool: {}", tool_name) }]),
    }
}