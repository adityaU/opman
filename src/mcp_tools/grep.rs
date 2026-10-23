//! `grep`: search file contents with a regex, respecting `.gitignore`.

use super::grep_scan::{scan_file, FileScan, Want};
use super::output::{ToolError, ToolOutput, ToolResult};
use super::walk::{files, Cancel};
use super::workspace::Workspace;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

const DEFAULT_LIMIT: usize = 250;
const MAX_LIMIT: usize = 2000;
const MAX_CONTEXT: usize = 20;

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaseMode {
    /// Upper and lower case differ.
    #[default]
    Sensitive,
    Insensitive,
    /// Ignore case unless the pattern has a capital letter.
    Smart,
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// File paths that contain a match.
    #[default]
    Files,
    /// Matching lines, with line numbers.
    Content,
    /// Match counts per file.
    Count,
}

#[derive(Debug, Deserialize)]
pub struct GrepArgs {
    #[serde(alias = "query", alias = "search_pattern")]
    pattern: String,
    #[serde(alias = "search_path", alias = "directory")]
    path: Option<String>,
    glob: Option<String>,
    #[serde(default)]
    case: CaseMode,
    context: Option<u64>,
    #[serde(default)]
    output_mode: OutputMode,
    head_limit: Option<u64>,
}

#[derive(Serialize)]
struct Item {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<u64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    context: bool,
}

#[derive(Serialize)]
struct GrepData {
    mode: OutputMode,
    results: Vec<Item>,
    truncated: bool,
}

pub async fn run(workspace: &Workspace, args: GrepArgs) -> ToolResult {
    let ignore_case = match args.case {
        CaseMode::Insensitive => true,
        CaseMode::Smart => !args.pattern.chars().any(char::is_uppercase),
        CaseMode::Sensitive => false,
    };
    let regex = RegexBuilder::new(&args.pattern)
        .case_insensitive(ignore_case)
        .build()
        .map_err(|e| ToolError::new(format!("invalid regex: {e}")))?;
    let root = match args.path.as_deref() {
        Some(raw) => workspace.resolve(raw)?,
        None => workspace.root().to_path_buf(),
    };
    let shown_root = workspace.show(&root);
    if let Err(e) = tokio::fs::metadata(&root).await {
        return Err(ToolError::io("search", &shown_root, &e));
    }
    let limit = usize::try_from(args.head_limit.unwrap_or(DEFAULT_LIMIT as u64))
        .unwrap_or(MAX_LIMIT)
        .clamp(1, MAX_LIMIT);
    let context = usize::try_from(args.context.unwrap_or(0))
        .unwrap_or(MAX_CONTEXT)
        .min(MAX_CONTEXT);
    let want = match args.output_mode {
        OutputMode::Files => Want::Any,
        OutputMode::Count => Want::Count,
        OutputMode::Content => Want::Lines { context },
    };
    let cancel = Cancel::default();
    let _guard = cancel.on_drop();
    let mode = args.output_mode;
    let glob = args.glob;
    let walk_root = root.clone();
    let (results, truncated) = tokio::task::spawn_blocking(move || {
        search(
            &walk_root,
            glob.as_deref(),
            &regex,
            want,
            mode,
            limit,
            &cancel,
        )
    })
    .await
    .map_err(|e| ToolError::new(format!("search failed: {e}")))??;

    let mut text = render(&results, mode);
    if results.is_empty() {
        text.push_str("No matches found");
    }
    if truncated {
        let _ = write!(
            text,
            "\n[stopped after {limit} results; narrow the search or raise head_limit]"
        );
    }
    ToolOutput::ok(
        text,
        &GrepData {
            mode,
            results,
            truncated,
        },
    )
}

fn search(
    root: &std::path::Path,
    glob: Option<&str>,
    regex: &regex::Regex,
    want: Want,
    mode: OutputMode,
    limit: usize,
    cancel: &Cancel,
) -> Result<(Vec<Item>, bool), ToolError> {
    let base = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let mut results = Vec::new();
    for entry in files(root, glob, cancel)? {
        let room = limit - results.len();
        let Ok(Some(scan)) = scan_file(entry.path(), regex, want, room) else {
            continue;
        };
        if scan.matches == 0 {
            continue;
        }
        let path = entry.path();
        let shown = path
            .strip_prefix(base)
            .unwrap_or(path)
            .display()
            .to_string();
        results.extend(items(shown, scan, mode));
        if results.len() >= limit {
            return Ok((results, true));
        }
    }
    Ok((results, false))
}

fn items(path: String, scan: FileScan, mode: OutputMode) -> Vec<Item> {
    let plain = |path: String| Item {
        path,
        line: None,
        text: None,
        count: None,
        context: false,
    };
    match mode {
        OutputMode::Files => vec![plain(path)],
        OutputMode::Count => vec![Item {
            count: Some(scan.matches),
            ..plain(path)
        }],
        OutputMode::Content => scan
            .hits
            .into_iter()
            .map(|hit| Item {
                line: Some(hit.line),
                text: Some(hit.text),
                context: !hit.is_match,
                ..plain(path.clone())
            })
            .collect(),
    }
}

/// `path:line:text` for matches, `path-line-text` for context, like ripgrep.
fn render(results: &[Item], mode: OutputMode) -> String {
    let mut text = String::new();
    let grouped = results.iter().any(|item| item.context);
    let mut last: Option<(&str, u64)> = None;
    for item in results {
        match mode {
            OutputMode::Files => text.push_str(&item.path),
            OutputMode::Count => {
                let _ = write!(text, "{}:{}", item.path, item.count.unwrap_or(0));
            }
            OutputMode::Content => {
                let line = item.line.unwrap_or(0);
                if grouped && last.is_some_and(|(p, l)| p != item.path || l + 1 != line) {
                    text.push_str("--\n");
                }
                last = Some((&item.path, line));
                let sep = if item.context { '-' } else { ':' };
                let body = item.text.as_deref().unwrap_or("");
                let _ = write!(text, "{}{sep}{line}{sep}{body}", item.path);
            }
        }
        text.push('\n');
    }
    text.truncate(text.trim_end().len());
    text
}
