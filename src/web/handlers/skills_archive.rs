//! Shared, bounded ZIP import for Settings upload and the management MCP.
use axum::{extract::State, Json};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Component, Path};

use crate::web::{
    auth::AuthUser,
    error::{WebError, WebResult},
    types::{ServerState, WebEvent},
};

const MAX_ARCHIVE: usize = 16 * 1024 * 1024;
const MAX_EXPANDED: u64 = 32 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Import {
    archive_base64: String,
}

pub async fn import_skills(
    _auth: AuthUser,
    State(state): State<ServerState>,
    Json(body): Json<Import>,
) -> WebResult<Json<Value>> {
    if body.archive_base64.len() > MAX_ARCHIVE * 4 / 3 + 4 {
        return Err(WebError::BadRequest("Skills ZIP is too large".into()));
    }
    let bytes = STANDARD
        .decode(body.archive_base64)
        .map_err(|_| WebError::BadRequest("Invalid base64 ZIP".into()))?;
    let count =
        tokio::task::spawn_blocking(move || extract(&bytes, &crate::mcp_skills::get_skills_dir()))
            .await
            .map_err(|_| WebError::Internal("Skills import failed".into()))?
            .map_err(WebError::BadRequest)?;
    let _ = state.reload_tx.send(());
    let _ = state.event_tx.send(WebEvent::SettingsChanged {
        section: "skills".into(),
    });
    Ok(Json(json!({"status":"uploaded","files":count})))
}

pub(super) fn extract(bytes: &[u8], root: &Path) -> Result<usize, String> {
    if bytes.len() > MAX_ARCHIVE {
        return Err("Skills ZIP is too large".into());
    }
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "Invalid skills ZIP")?;
    if archive.len() > 512 {
        return Err("Skills ZIP has too many entries".into());
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| "Cannot read skills ZIP entry")?;
        file.enclosed_name()
            .ok_or("Skills ZIP has an invalid path")?;
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Skills ZIP cannot contain symbolic links".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("Skills ZIP is too large")?;
        if total > MAX_EXPANDED {
            return Err("Expanded skills ZIP is too large".into());
        }
    }
    std::fs::create_dir_all(root).map_err(|_| "Cannot create skills directory")?;
    let mut written = 0;
    let mut expanded = 0_u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| "Cannot read skills ZIP entry")?;
        let relative = file
            .enclosed_name()
            .ok_or("Skills ZIP has an invalid path")?;
        let target = root.join(&relative);
        if file.is_dir() {
            ensure_directory(root, &relative)?;
            continue;
        }
        ensure_directory(root, relative.parent().ok_or("Invalid skills ZIP path")?)?;
        if std::fs::symlink_metadata(&target)
            .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err("Skills ZIP cannot overwrite a link or directory".into());
        }
        let mut output =
            std::fs::File::create(target).map_err(|_| "Cannot write imported skill")?;
        let remaining = MAX_EXPANDED - expanded;
        let mut bounded = std::io::Read::take(&mut file, remaining + 1);
        let count = std::io::copy(&mut bounded, &mut output)
            .map_err(|_| "Cannot extract imported skill")?;
        if count > remaining {
            return Err("Expanded skills ZIP is too large".into());
        }
        expanded += count;
        written += 1;
    }
    Ok(written)
}

fn ensure_directory(root: &Path, relative: &Path) -> Result<(), String> {
    let mut directory = root.to_path_buf();
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err("Invalid skills ZIP path".into());
        };
        directory.push(name);
        match std::fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err("Skills ZIP path crosses a symbolic link or file".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&directory)
                    .map_err(|_| "Cannot create imported skill directory")?
            }
            Err(_) => return Err("Cannot inspect imported skill directory".into()),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "skills_archive_tests.rs"]
mod tests;
