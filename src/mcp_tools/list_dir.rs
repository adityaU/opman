//! `list_dir`: the entries of one folder, folders first.

use super::output::{ToolError, ToolOutput, ToolResult};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 1000;

#[derive(Debug, Default, Deserialize)]
pub struct ListArgs {
    path: Option<String>,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Dir,
    File,
    Symlink,
    Other,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}

#[derive(Serialize)]
struct ListData {
    path: String,
    entries: Vec<Entry>,
    truncated: bool,
}

pub async fn run(workspace: &Workspace, args: ListArgs) -> ToolResult {
    let path = match args.path.as_deref() {
        Some(raw) => workspace.resolve(raw)?,
        None => workspace.root().to_path_buf(),
    };
    let shown = workspace.show(&path);
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| ToolError::io("list", &shown, &e))?;
    let mut entries = Vec::new();
    let mut truncated = false;
    while let Some(item) = dir
        .next_entry()
        .await
        .map_err(|e| ToolError::io("list", &shown, &e))?
    {
        if entries.len() == MAX_ENTRIES {
            truncated = true;
            break;
        }
        let file_type = item.file_type().await.ok();
        let kind = match file_type {
            Some(t) if t.is_dir() => Kind::Dir,
            Some(t) if t.is_symlink() => Kind::Symlink,
            Some(t) if t.is_file() => Kind::File,
            _ => Kind::Other,
        };
        let size = match kind {
            Kind::File => item.metadata().await.ok().map(|m| m.len()),
            _ => None,
        };
        entries.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            kind,
            size,
        });
    }
    entries.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name)));

    let mut text = String::new();
    for entry in &entries {
        text.push_str(&entry.name);
        if entry.kind == Kind::Dir {
            text.push('/');
        }
        text.push('\n');
    }
    if entries.is_empty() {
        text.push_str("(empty folder)");
    }
    if truncated {
        text.push_str(&format!("[only the first {MAX_ENTRIES} entries are shown]"));
    }
    ToolOutput::ok(
        text,
        &ListData {
            path: shown,
            entries,
            truncated,
        },
    )
}
