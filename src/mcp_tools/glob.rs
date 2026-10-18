//! `glob`: find files by name pattern, respecting `.gitignore`.

use super::output::{ToolError, ToolOutput, ToolResult};
use super::walk::{files, Cancel};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};

const MAX_FILES: usize = 1000;

#[derive(Debug, Deserialize)]
pub struct GlobArgs {
    pattern: String,
    path: Option<String>,
}

#[derive(Serialize)]
struct GlobData {
    root: String,
    files: Vec<String>,
    truncated: bool,
}

pub async fn run(workspace: &Workspace, args: GlobArgs) -> ToolResult {
    let root = match args.path.as_deref() {
        Some(raw) => workspace.resolve(raw)?,
        None => workspace.root().to_path_buf(),
    };
    let shown = workspace.show(&root);
    match tokio::fs::metadata(&root).await {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(ToolError::new(format!("{shown} is not a folder"))),
        Err(e) => return Err(ToolError::io("search", &shown, &e)),
    }
    let cancel = Cancel::default();
    let _guard = cancel.on_drop();
    let pattern = args.pattern;
    let found = {
        let root = root.clone();
        tokio::task::spawn_blocking(move || -> Result<(Vec<String>, bool), ToolError> {
            let mut list = Vec::new();
            for entry in files(&root, Some(&pattern), &cancel)? {
                if list.len() == MAX_FILES {
                    return Ok((list, true));
                }
                let path = entry.path();
                list.push(
                    path.strip_prefix(&root)
                        .unwrap_or(path)
                        .display()
                        .to_string(),
                );
            }
            Ok((list, false))
        })
        .await
        .map_err(|e| ToolError::new(format!("search failed: {e}")))??
    };
    let (list, truncated) = found;
    let mut text = list.join("\n");
    if list.is_empty() {
        text.push_str("No files matched");
    }
    if truncated {
        text.push_str(&format!("\n[only the first {MAX_FILES} files are shown]"));
    }
    ToolOutput::ok(
        text,
        &GlobData {
            root: shown,
            files: list,
            truncated,
        },
    )
}
