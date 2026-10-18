//! `edit_file`: replace exact text in a file, and show a short diff of the change.

use super::atomic::atomic_write;
use super::output::{ToolError, ToolOutput, ToolResult};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::fmt::Write;

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_DIFF_LINES: usize = 40;

#[derive(Debug, Deserialize)]
pub struct EditArgs {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Serialize)]
struct EditData {
    path: String,
    replacements: usize,
    first_line: usize,
    diff: String,
}

pub async fn run(workspace: &Workspace, args: EditArgs) -> ToolResult {
    if args.old_string.is_empty() {
        return Err(ToolError::new(
            "old_string is empty; use write_file to create or replace a whole file",
        ));
    }
    if args.old_string == args.new_string {
        return Err(ToolError::new("old_string and new_string are the same"));
    }
    let path = workspace.resolve(&args.path)?;
    let shown = workspace.show(&path);
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| ToolError::io("edit", &shown, &e))?;
    if !meta.is_file() {
        return Err(ToolError::new(format!("{shown} is not a regular file")));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(ToolError::new(format!(
            "{shown} is larger than {} MB; edit it with bash instead",
            MAX_FILE_BYTES / 1024 / 1024
        )));
    }
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| ToolError::io("read", &shown, &e))?;

    let mut found = text.match_indices(args.old_string.as_str());
    let Some((first_at, _)) = found.next() else {
        return Err(ToolError::new(format!(
            "old_string was not found in {shown}. It must match exactly, including spaces, \
             tabs and line breaks. Read the file again and copy the text."
        )));
    };
    let replacements = 1 + found.count();
    if replacements > 1 && !args.replace_all {
        return Err(ToolError::new(format!(
            "old_string appears {replacements} times in {shown}. Add more surrounding lines to \
             make it unique, or set replace_all to true to change every one."
        )));
    }
    let edited = if args.replace_all {
        text.replace(args.old_string.as_str(), &args.new_string)
    } else {
        text.replacen(args.old_string.as_str(), &args.new_string, 1)
    };
    let target = tokio::fs::canonicalize(&path).await.unwrap_or(path);
    atomic_write(&target, edited.as_bytes())
        .await
        .map_err(|e| ToolError::io("write", &shown, &e))?;

    let first_line = text[..first_at].matches('\n').count() + 1;
    let diff = short_diff(&args.old_string, &args.new_string, first_line);
    let summary = match replacements {
        1 => format!("Edited {shown}"),
        n => format!("Edited {shown} ({n} places)"),
    };
    let data = EditData {
        path: shown,
        replacements,
        first_line,
        diff: diff.clone(),
    };
    ToolOutput::ok(format!("{summary}\n{diff}"), &data)
}

/// The changed text only, not the whole file: `-` old lines, `+` new lines.
fn short_diff(old: &str, new: &str, first_line: usize) -> String {
    let mut out = format!("@@ line {first_line} @@\n");
    let diff = TextDiff::from_lines(old, new);
    let mut shown = 0;
    for change in diff.iter_all_changes() {
        if shown == MAX_DIFF_LINES {
            out.push_str("... diff cut\n");
            break;
        }
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        let line = change.value();
        let _ = write!(out, "{sign}{line}");
        if !line.ends_with('\n') {
            out.push('\n');
        }
        shown += 1;
    }
    out
}
