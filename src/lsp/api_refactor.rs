//! References and rename — the two operations that ask about a symbol across
//! the whole project rather than at one point in one file.
//!
//! Rename is the only LSP operation here that writes files opman was not asked
//! about: a workspace edit can touch any file the server chose. Every target is
//! therefore re-read from disk, edited, written, and re-synced individually, and
//! a file whose edits do not apply cleanly is skipped rather than truncated.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use tracing::debug;

use super::api::{prepare, read_text, unavailable};
use super::convert::{
    apply_text_edits, from_lsp_position, path_to_uri, to_lsp_position, uri_to_path,
    PositionEncoding,
};
use super::pool::LspPool;
use super::server::QUERY_TIMEOUT;

// ── References ──────────────────────────────────────────

pub async fn references(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    line: i64,
    col: i64,
    content: Option<&str>,
) -> Value {
    let Some((resolved, caps)) = prepare(pool, file, project_dir, content).await else {
        return unavailable(&[("locations", json!([]))]);
    };
    if !caps.references {
        return unavailable(&[("locations", json!([]))]);
    }
    let text = read_text(file, content);
    let position = to_lsp_position(&text, line, col, caps.encoding);
    let result = resolved
        .server
        .peer
        .request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": path_to_uri(file) },
                "position": position,
                "context": { "includeDeclaration": true },
            }),
            QUERY_TIMEOUT,
        )
        .await;
    let Ok(Value::Array(found)) = result else {
        debug!("lsp: references failed");
        return unavailable(&[("locations", json!([]))]);
    };
    let locations: Vec<Value> = found
        .iter()
        .filter_map(|value| location(value, caps.encoding))
        .collect();
    json!({ "available": true, "locations": locations })
}

fn location(value: &Value, encoding: PositionEncoding) -> Option<Value> {
    let uri = value.get("uri")?.as_str()?;
    let start = value.get("range")?.get("start")?;
    let target = uri_to_path(uri)?;
    let text = std::fs::read_to_string(&target).unwrap_or_default();
    let (lnum, col) = from_lsp_position(&text, start, encoding);
    let preview = text
        .lines()
        .nth(lnum.saturating_sub(1) as usize)
        .unwrap_or_default()
        .trim()
        .to_owned();
    Some(json!({
        "file": target.to_string_lossy(),
        "lnum": lnum,
        "col": col,
        "text": preview,
    }))
}

// ── Rename ──────────────────────────────────────────────

pub async fn rename(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    line: i64,
    col: i64,
    new_name: &str,
    content: Option<&str>,
) -> Value {
    if new_name.trim().is_empty() {
        return json!({ "available": true, "renamed": false, "files": [] });
    }
    let Some((resolved, caps)) = prepare(pool, file, project_dir, content).await else {
        return unavailable(&[("renamed", json!(false)), ("files", json!([]))]);
    };
    if !caps.rename {
        return unavailable(&[("renamed", json!(false)), ("files", json!([]))]);
    }
    let text = read_text(file, content);
    let position = to_lsp_position(&text, line, col, caps.encoding);
    let result = resolved
        .server
        .peer
        .request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": path_to_uri(file) },
                "position": position,
                "newName": new_name,
            }),
            QUERY_TIMEOUT,
        )
        .await;
    let Ok(edit) = result else {
        debug!("lsp: rename failed");
        return unavailable(&[("renamed", json!(false)), ("files", json!([]))]);
    };

    let mut written: Vec<String> = Vec::new();
    for (target, edits) in workspace_edits(&edit) {
        if !target.starts_with(project_dir) {
            debug!("lsp: rename touched a file outside the project; skipped");
            continue;
        }
        let original = std::fs::read_to_string(&target).unwrap_or_default();
        let Some(updated) = apply_text_edits(&original, &edits, caps.encoding) else {
            debug!("lsp: rename edits did not apply cleanly");
            continue;
        };
        if updated == original || std::fs::write(&target, &updated).is_err() {
            continue;
        }
        let _ = resolved.server.docs.sync(
            &resolved.server.peer,
            &target,
            resolved.spec.language,
            Some(&updated),
        );
        resolved
            .server
            .docs
            .notify_saved(&resolved.server.peer, &target);
        written.push(target.to_string_lossy().into_owned());
    }
    json!({ "available": true, "renamed": !written.is_empty(), "files": written })
}

/// Flatten either shape of `WorkspaceEdit` into path/edit-list pairs.
fn workspace_edits(edit: &Value) -> Vec<(PathBuf, Vec<Value>)> {
    if let Some(changes) = edit.get("changes").and_then(Value::as_object) {
        return changes
            .iter()
            .filter_map(|(uri, edits)| Some((uri_to_path(uri)?, edits.as_array()?.clone())))
            .collect();
    }
    edit.get("documentChanges")
        .and_then(Value::as_array)
        .map(|documents| {
            documents
                .iter()
                .filter_map(|document| {
                    let uri = document.get("textDocument")?.get("uri")?.as_str()?;
                    Some((
                        uri_to_path(uri)?,
                        document.get("edits")?.as_array()?.clone(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}
