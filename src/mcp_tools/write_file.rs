//! `write_file`: create or replace a whole file atomically.

use super::atomic::atomic_write;
use super::output::{ToolError, ToolOutput, ToolResult};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct WriteArgs {
    path: String,
    content: String,
}

#[derive(Serialize)]
struct WriteData {
    path: String,
    bytes: usize,
    created: bool,
}

pub async fn run(workspace: &Workspace, args: WriteArgs) -> ToolResult {
    let path = workspace.resolve(&args.path)?;
    let shown = workspace.show(&path);
    let existing = tokio::fs::metadata(&path).await.ok();
    if existing.as_ref().is_some_and(|m| m.is_dir()) {
        return Err(ToolError::new(format!("{shown} is a directory")));
    }
    // Write through a symlink instead of replacing the link itself.
    let target = match existing {
        Some(_) => tokio::fs::canonicalize(&path).await.unwrap_or(path),
        None => path,
    };
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ToolError::io("create the folder for", &shown, &e))?;
    }
    atomic_write(&target, args.content.as_bytes())
        .await
        .map_err(|e| ToolError::io("write", &shown, &e))?;
    let created = existing.is_none();
    let verb = if created { "Created" } else { "Overwrote" };
    let text = format!("{verb} {shown} ({} bytes)", args.content.len());
    let data = WriteData {
        path: shown,
        bytes: args.content.len(),
        created,
    };
    ToolOutput::ok(text, &data)
}
