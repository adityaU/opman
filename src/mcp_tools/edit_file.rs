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
pub struct EditHunk {
    #[serde(default, alias = "old_str", alias = "oldText")]
    old_string: String,
    #[serde(default, alias = "new_str", alias = "newText")]
    new_string: String,
    #[serde(default)]
    replace_all: bool,
    start_line: Option<usize>,
    end_line: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct EditArgs {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
    #[serde(default, alias = "old_str", alias = "oldText")]
    old_string: String,
    #[serde(default, alias = "new_str", alias = "newText")]
    new_string: String,
    #[serde(default)]
    replace_all: bool,
    start_line: Option<usize>,
    end_line: Option<usize>,
    edits: Option<Vec<EditHunk>>,
}

#[derive(Serialize)]
struct EditData {
    path: String,
    replacements: usize,
    first_line: usize,
    diff: String,
}

pub async fn run(workspace: &Workspace, args: EditArgs) -> ToolResult {
    let hunks = match args.edits {
        Some(list) if !list.is_empty() => list,
        Some(_) => return Err(ToolError::new("edits list must not be empty")),
        None => vec![EditHunk {
            old_string: args.old_string,
            new_string: args.new_string,
            replace_all: args.replace_all,
            start_line: args.start_line,
            end_line: args.end_line,
        }],
    };
    for hunk in &hunks {
        if hunk.start_line.is_none() {
            if hunk.old_string.is_empty() {
                return Err(ToolError::new(
                    "old_string is empty; use write_file to create or replace a whole file",
                ));
            }
            if hunk.old_string == hunk.new_string {
                return Err(ToolError::new("old_string and new_string are the same"));
            }
        }
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
    let mut edited = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| ToolError::io("read", &shown, &e))?;

    let mut total_replacements = 0usize;
    let mut earliest_line = usize::MAX;
    let mut diffs = Vec::new();

    for hunk in &hunks {
        let (next_text, count, line_no, diff) = apply_hunk(&edited, hunk, &shown)?;
        edited = next_text;
        total_replacements += count;
        earliest_line = earliest_line.min(line_no);
        diffs.push(diff);
    }

    let target = tokio::fs::canonicalize(&path).await.unwrap_or(path);
    atomic_write(&target, edited.as_bytes())
        .await
        .map_err(|e| ToolError::io("write", &shown, &e))?;

    let first_line = if earliest_line == usize::MAX {
        1
    } else {
        earliest_line
    };
    let diff = diffs.join("");
    let summary = match total_replacements {
        1 => format!("Edited {shown}"),
        n => format!("Edited {shown} ({n} places)"),
    };
    let data = EditData {
        path: shown,
        replacements: total_replacements,
        first_line,
        diff: diff.clone(),
    };
    ToolOutput::ok(format!("{summary}\n{diff}"), &data)
}

fn apply_hunk(
    text: &str,
    hunk: &EditHunk,
    shown: &str,
) -> Result<(String, usize, usize, String), ToolError> {
    if let Some(start) = hunk.start_line {
        let start = start.max(1);
        let end = hunk.end_line.unwrap_or(start).max(start);
        let (byte_start, byte_end, total_lines) = line_byte_range(text, start, end)
            .ok_or_else(|| {
                ToolError::new(format!(
                    "line range {start}..{end} is out of bounds for {shown}"
                ))
            })?;
        let old_slice = &text[byte_start..byte_end];
        let mut replacement = hunk.new_string.clone();
        if old_slice.ends_with('\n') && !replacement.is_empty() && !replacement.ends_with('\n') {
            if old_slice.ends_with("\r\n") {
                replacement.push_str("\r\n");
            } else {
                replacement.push('\n');
            }
        }
        if old_slice == replacement {
            return Err(ToolError::new("old_string and new_string are the same"));
        }
        let _ = total_lines;
        let mut out = String::with_capacity(text.len() - old_slice.len() + replacement.len());
        out.push_str(&text[..byte_start]);
        out.push_str(&replacement);
        out.push_str(&text[byte_end..]);
        let diff = short_diff(old_slice, &replacement, start);
        return Ok((out, 1, start, diff));
    }

    // Try exact match first; fall back to CRLF-normalized match if the file uses `\r\n`.
    let (old_str, new_str) = if text.contains(&hunk.old_string)
        || !text.contains("\r\n")
        || hunk.old_string.contains("\r\n")
    {
        (hunk.old_string.clone(), hunk.new_string.clone())
    } else {
        let crlf_old = hunk.old_string.replace('\n', "\r\n");
        if text.contains(&crlf_old) {
            (crlf_old, hunk.new_string.replace('\n', "\r\n"))
        } else {
            (hunk.old_string.clone(), hunk.new_string.clone())
        }
    };

    let mut found = text.match_indices(old_str.as_str());
    let Some((first_at, _)) = found.next() else {
        return Err(ToolError::new(format!(
            "old_string was not found in {shown}. It must match exactly, including spaces, \
             tabs and line breaks. Read the file again and copy the text."
        )));
    };
    let replacements = 1 + found.count();
    if replacements > 1 && !hunk.replace_all {
        return Err(ToolError::new(format!(
            "old_string appears {replacements} times in {shown}. Add more surrounding lines to \
             make it unique, or set replace_all to true to change every one."
        )));
    }
    let edited = if hunk.replace_all {
        text.replace(old_str.as_str(), &new_str)
    } else {
        text.replacen(old_str.as_str(), &new_str, 1)
    };
    let first_line = text[..first_at].matches('\n').count() + 1;
    let diff = short_diff(&hunk.old_string, &hunk.new_string, first_line);
    Ok((edited, replacements, first_line, diff))
}

fn line_byte_range(text: &str, start_line: usize, end_line: usize) -> Option<(usize, usize, usize)> {
    let mut line = 1usize;
    let mut byte_start = if start_line == 1 { Some(0) } else { None };
    let mut byte_end = None;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            if line == end_line {
                byte_end = Some(idx + 1);
                break;
            }
            line += 1;
            if line == start_line {
                byte_start = Some(idx + 1);
            }
        }
    }
    let s = byte_start?;
    let e = byte_end.unwrap_or_else(|| if end_line >= line { text.len() } else { s });
    if start_line > line {
        return None;
    }
    Some((s, e, line))
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
