use axum::extract::{Multipart, State, Path};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::web::types::ServerState;
use crate::mcp_skills::Skill;

#[derive(Serialize)]
pub struct SkillSummary {
    name: String,
    description: String,
}

#[derive(Deserialize)]
pub struct CreateSkillRequest {
    name: String,
    description: String,
    content: String,
}

pub async fn list_skills(
    State(state): State<ServerState>,
) -> Result<Json<Vec<SkillSummary>>, StatusCode> {
    let registry = state.skills_registry.read().await;
    let skills: Vec<SkillSummary> = registry.values().map(|s| SkillSummary {
        name: s.name.clone(),
        description: s.description.clone(),
    }).collect();
    Ok(Json(skills))
}

pub async fn get_skill(
    State(state): State<ServerState>,
    Path(name): Path<String>,
) -> Result<Json<Option<Skill>>, StatusCode> {
    let registry = state.skills_registry.read().await;
    Ok(Json(registry.get(&name).cloned()))
}

pub async fn create_skill(
    State(state): State<ServerState>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.name.is_empty() || req.description.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let skill_dir = crate::mcp_skills::get_skills_dir().join(&req.name);
    std::fs::create_dir_all(&skill_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let skill_md = skill_dir.join("SKILL.md");
    let content = format!("---\nname: {}\ndescription: {}\n---\n{}", req.name, req.description, req.content);
    std::fs::write(&skill_md, content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.reload_tx.send(());
    Ok(Json(json!({"status": "created"})))
}

pub async fn update_skill(
    State(state): State<ServerState>,
    Path(name): Path<String>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let skill_dir = crate::mcp_skills::get_skills_dir().join(&name);
    if !skill_dir.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    let skill_md = skill_dir.join("SKILL.md");
    let content = format!("---\nname: {}\ndescription: {}\n---\n{}", req.name, req.description, req.content);
    std::fs::write(&skill_md, content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.reload_tx.send(());
    Ok(Json(json!({"status": "updated"})))
}

pub async fn delete_skill(
    State(state): State<ServerState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let skill_dir = crate::mcp_skills::get_skills_dir().join(&name);
    if !skill_dir.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    std::fs::remove_dir_all(&skill_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.reload_tx.send(());
    Ok(Json(json!({"status": "deleted"})))
}

pub async fn upload_skills(
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        if field.name() == Some("skills_zip") {
            let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            let skills_dir = crate::mcp_skills::get_skills_dir();

            // Extract ZIP
            let cursor = std::io::Cursor::new(data);
            let mut archive = zip::ZipArchive::new(cursor).map_err(|_| StatusCode::BAD_REQUEST)?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let outpath = skills_dir.join(file.name());

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath).ok();
                } else {
                    if let Some(p) = outpath.parent() {
                        std::fs::create_dir_all(p).ok();
                    }
                    let mut outfile = std::fs::File::create(&outpath).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    std::io::copy(&mut file, &mut outfile).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }

            // Send reload signal
            let _ = state.reload_tx.send(());

            return Ok(Json(json!({"status": "uploaded"})));
        }
    }

    Err(StatusCode::BAD_REQUEST)
}