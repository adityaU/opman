//! `read_file`: numbered lines from a text file, with caps on lines and bytes.

use super::limits::ByteLimit;
use super::lines::read_line_capped;
use super::output::{ToolError, ToolOutput, ToolResult};
use super::workspace::Workspace;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

const DEFAULT_LINES: usize = 2000;
const MAX_LINES: usize = 10_000;
const OUTPUT_LIMIT: ByteLimit = ByteLimit::new(256 * 1024);
const LINE_LIMIT: usize = 2000;

#[derive(Debug, Deserialize)]
pub struct ReadArgs {
    path: String,
    /// First line to return, counting from 1.
    offset: Option<u64>,
    limit: Option<u64>,
}

#[derive(Serialize)]
struct ReadData {
    path: String,
    start_line: u64,
    end_line: u64,
    content: String,
    has_more: bool,
    long_lines_clipped: bool,
}

pub async fn run(workspace: &Workspace, args: ReadArgs) -> ToolResult {
    let path = workspace.resolve(&args.path)?;
    let shown = workspace.show(&path);
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| ToolError::io("read", &shown, &e))?;
    if meta.is_dir() {
        return Err(ToolError::new(format!(
            "{shown} is a directory; use list_dir"
        )));
    }
    let file = File::open(&path)
        .await
        .map_err(|e| ToolError::io("read", &shown, &e))?;
    let mut reader = BufReader::new(file);
    let head = reader
        .fill_buf()
        .await
        .map_err(|e| ToolError::io("read", &shown, &e))?;
    if head.contains(&0) {
        let text = format!("{shown}: binary file ({} bytes). Open the file preview to view supported images, video, audio, PDFs and STL models.", meta.len());
        let data = serde_json::json!({
            "path": shown,
            "start_line": 0,
            "end_line": 0,
            "content": "",
            "has_more": false,
            "long_lines_clipped": false,
            "binary": true,
            "bytes": meta.len(),
        });
        return ToolOutput::ok(text, &data);
    }

    let first = args.offset.unwrap_or(1).max(1);
    let wanted = usize::try_from(args.limit.unwrap_or(DEFAULT_LINES as u64))
        .unwrap_or(MAX_LINES)
        .clamp(1, MAX_LINES);
    let mut numbered = String::new();
    let mut raw = String::new();
    let mut line_no = 0u64;
    let mut clipped_any = false;
    let mut buf = Vec::new();
    let mut has_more = false;
    loop {
        let clipped = match read_line_capped(&mut reader, &mut buf, LINE_LIMIT)
            .await
            .map_err(|e| ToolError::io("read", &shown, &e))?
        {
            Some(clipped) => clipped,
            None => break,
        };
        line_no += 1;
        if line_no < first {
            continue;
        }
        if line_no - first >= wanted as u64 || numbered.len() >= OUTPUT_LIMIT.get() {
            has_more = true;
            break;
        }
        clipped_any |= clipped;
        let line = String::from_utf8_lossy(&buf);
        let _ = writeln!(
            numbered,
            "{line_no:>6}\t{line}{}",
            if clipped { " [line cut]" } else { "" }
        );
        raw.push_str(&line);
        raw.push('\n');
    }
    if numbered.is_empty() {
        if line_no == 0 {
            return ToolOutput::ok("(empty file)".into(), &empty(&shown));
        }
        return Err(ToolError::new(format!(
            "offset {first} is past the end of {shown}, which has {line_no} lines"
        )));
    }
    let end_line = first + raw.matches('\n').count() as u64 - 1;
    if has_more {
        let _ = write!(
            numbered,
            "[more lines follow; call again with offset {}]",
            end_line + 1
        );
    }
    let data = ReadData {
        path: shown,
        start_line: first,
        end_line,
        content: raw,
        has_more,
        long_lines_clipped: clipped_any,
    };
    ToolOutput::ok(numbered, &data)
}

fn empty(shown: &str) -> ReadData {
    ReadData {
        path: shown.to_string(),
        start_line: 0,
        end_line: 0,
        content: String::new(),
        has_more: false,
        long_lines_clipped: false,
    }
}
