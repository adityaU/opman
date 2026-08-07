use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::mcp_skills::format::{render_skill_md, SkillDraft};
use crate::mcp_skills::{Skill, SkillName};
use crate::web::auth::AuthUser;
use crate::web::types::ServerState;

#[derive(Serialize)]
pub struct SkillSummary {
    name: SkillName,
    title: String,
    description: String,
    requires: Vec<String>,
}

/// `name` deserializes through [`SkillName`], so a traversal attempt is a 422 at the
/// extractor rather than something each handler has to remember to check.
#[derive(Deserialize)]
pub struct CreateSkillRequest {
    name: SkillName,
    #[serde(default)]
    title: String,
    description: String,
    content: String,
    #[serde(default)]
    requires: Vec<String>,
}

impl CreateSkillRequest {
    fn render(&self) -> Result<String, StatusCode> {
        render_skill_md(&SkillDraft {
            name: &self.name,
            title: (!self.title.is_empty()).then_some(self.title.as_str()),
            description: &self.description,
            requires: &self.requires,
            body: &self.content,
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

pub async fn list_skills(
    _auth: AuthUser,
    State(state): State<ServerState>,
) -> Result<Json<Vec<SkillSummary>>, StatusCode> {
    let registry = state.skills_registry.read().await;
    let skills: Vec<SkillSummary> = registry
        .values()
        .map(|s| SkillSummary {
            name: s.name.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            requires: s.requires.clone(),
        })
        .collect();
    Ok(Json(skills))
}

pub async fn get_skill(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<SkillName>,
) -> Result<Json<Option<Skill>>, StatusCode> {
    let registry = state.skills_registry.read().await;
    Ok(Json(registry.get(&name).cloned()))
}

pub async fn create_skill(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.description.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let skill_dir = req.name.dir_in(&crate::mcp_skills::get_skills_dir());
    std::fs::create_dir_all(&skill_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::write(skill_dir.join("SKILL.md"), req.render()?)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.reload_tx.send(());
    Ok(Json(json!({"status": "created"})))
}

pub async fn update_skill(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<SkillName>,
    Json(req): Json<CreateSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let skill_dir = name.dir_in(&crate::mcp_skills::get_skills_dir());
    if !skill_dir.is_dir() {
        return Err(StatusCode::NOT_FOUND);
    }
    std::fs::write(skill_dir.join("SKILL.md"), req.render()?)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.reload_tx.send(());
    Ok(Json(json!({"status": "updated"})))
}

pub async fn delete_skill(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Path(name): Path<SkillName>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let skill_dir = name.dir_in(&crate::mcp_skills::get_skills_dir());
    if !skill_dir.is_dir() {
        return Err(StatusCode::NOT_FOUND);
    }
    std::fs::remove_dir_all(&skill_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.reload_tx.send(());
    Ok(Json(json!({"status": "deleted"})))
}

pub async fn upload_skills(
    _auth: AuthUser,
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        if field.name() == Some("skills_zip") {
            let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            let skills_dir = crate::mcp_skills::get_skills_dir();

            // Extract ZIP
            let cursor = std::io::Cursor::new(data);
            let mut archive = zip::ZipArchive::new(cursor).map_err(|_| StatusCode::BAD_REQUEST)?;

            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let outpath = skills_dir.join(file.name());

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath).ok();
                } else {
                    if let Some(p) = outpath.parent() {
                        std::fs::create_dir_all(p).ok();
                    }
                    let mut outfile = std::fs::File::create(&outpath)
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    std::io::copy(&mut file, &mut outfile)
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }

            // Send reload signal
            let _ = state.reload_tx.send(());

            return Ok(Json(json!({"status": "uploaded"})));
        }
    }

    Err(StatusCode::BAD_REQUEST)
}

#[cfg(test)]
#[path = "skills_handlers_tests.rs"]
mod skills_handlers_tests;

#[cfg(test)]
#[path = "skills_handlers_fs_tests.rs"]
mod skills_handlers_fs_tests;
