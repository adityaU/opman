//! The operations the editor asks for, in the shapes it already expects.
//!
//! Every one of them can fail in a way that is not an error: the file type has
//! no server, the server is not installed, it is still starting, it crashed. In
//! all of those the honest answer is "no LSP here" — `available: false` — and
//! never a 500, because the editor renders availability and a 500 would surface
//! as a scary toast for the ordinary case of opening a `.txt` file.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tracing::debug;

use super::convert::{from_lsp_position, hover_text, path_to_uri, PositionEncoding};
use super::diags::FIRST_PUBLISH_WAIT;
use super::pool::{LspPool, Resolved};
use super::server::QUERY_TIMEOUT;

/// The `available: false` answer, shared by every "not applicable" path.
pub(super) fn unavailable(extra: &[(&str, Value)]) -> Value {
    let mut out = json!({ "available": false });
    for (key, value) in extra {
        out[*key] = value.clone();
    }
    out
}

/// Resolve the server and make sure it knows the current text.
///
/// Returns `None` for every benign reason there is no LSP, so callers can map
/// that straight to `available: false`.
pub(super) async fn prepare(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    content: Option<&str>,
) -> Option<(Resolved, super::server::ServerCaps)> {
    let resolved = match pool.resolve(file, project_dir).await {
        Ok(Some(resolved)) => resolved,
        Ok(None) => return None,
        Err(e) => {
            debug!("lsp: {e}");
            return None;
        }
    };

    let caps = match resolved.server.ready().await {
        Ok(caps) => caps,
        Err(e) => {
            debug!("lsp: {e}");
            pool.evict(&resolved.key).await;
            return None;
        }
    };

    let language = resolved.spec.language;
    if let Err(e) = resolved
        .server
        .docs
        .sync(&resolved.server.peer, file, language, content)
    {
        debug!("lsp: document sync failed: {e}");
        return None;
    }
    Some((resolved, caps))
}

// ── Diagnostics ─────────────────────────────────────────

pub async fn diagnostics(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    content: Option<&str>,
) -> Value {
    let Some((resolved, _)) = prepare(pool, file, project_dir, content).await else {
        return unavailable(&[("diagnostics", json!([]))]);
    };
    let uri = path_to_uri(file);

    // Only the first ask for a file waits; after that whatever the server last
    // said is the truth, including "nothing wrong".
    let raw = match resolved.server.diags.get(&uri) {
        Some(found) => found,
        None => {
            resolved
                .server
                .diags
                .wait_for(&uri, FIRST_PUBLISH_WAIT)
                .await
        }
    };

    let text = content.map(str::to_string).unwrap_or_default();
    let encoding = PositionEncoding::Utf8;
    let diagnostics: Vec<Value> = raw
        .iter()
        .filter_map(|diag| render_diagnostic(diag, file, &text, encoding))
        .collect();

    json!({ "available": true, "diagnostics": diagnostics })
}

/// Map an LSP diagnostic into the `{file, lnum, col, severity, message, source}`
/// shape the editor renders.
fn render_diagnostic(
    diag: &Value,
    file: &Path,
    text: &str,
    encoding: PositionEncoding,
) -> Option<Value> {
    let start = diag.get("range")?.get("start")?;
    let (lnum, col) = from_lsp_position(text, start, encoding);
    let severity = match diag.get("severity").and_then(Value::as_i64) {
        Some(1) => "error",
        Some(2) => "warning",
        Some(3) => "info",
        Some(4) => "hint",
        _ => "error",
    };
    Some(json!({
        "file": file.to_string_lossy(),
        "lnum": lnum,
        "col": col,
        "severity": severity,
        "message": diag.get("message").and_then(Value::as_str).unwrap_or_default(),
        "source": diag.get("source").and_then(Value::as_str).unwrap_or_default(),
    }))
}

// ── Hover ───────────────────────────────────────────────

pub async fn hover(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    line: i64,
    col: i64,
    content: Option<&str>,
) -> Value {
    let Some((resolved, caps)) = prepare(pool, file, project_dir, content).await else {
        return unavailable(&[("hover", Value::Null)]);
    };
    if !caps.hover {
        return unavailable(&[("hover", Value::Null)]);
    }
    let text = read_text(file, content);
    let position = super::convert::to_lsp_position(&text, line, col, caps.encoding);

    let result = resolved
        .server
        .peer
        .request(
            "textDocument/hover",
            json!({ "textDocument": { "uri": path_to_uri(file) }, "position": position }),
            QUERY_TIMEOUT,
        )
        .await;

    match result {
        Ok(value) => json!({ "available": true, "hover": hover_text(&value) }),
        Err(e) => {
            debug!("lsp: hover failed: {e}");
            unavailable(&[("hover", Value::Null)])
        }
    }
}

// ── Completion ──────────────────────────────────────────

pub async fn completion(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    line: i64,
    col: i64,
    content: Option<&str>,
    trigger: Option<&str>,
) -> Value {
    let empty = || unavailable(&[("items", json!([])), ("triggerCharacters", json!([]))]);

    let Some((resolved, caps)) = prepare(pool, file, project_dir, content).await else {
        return empty();
    };
    if !caps.completion {
        return empty();
    }

    let text = read_text(file, content);
    let position = super::convert::to_lsp_position(&text, line, col, caps.encoding);

    // `context` tells the server whether this was typed into (a trigger
    // character) or asked for explicitly; rust-analyzer returns a different,
    // much shorter list for `.` than for a bare invocation.
    let context = match trigger {
        Some(character) => json!({ "triggerKind": 2, "triggerCharacter": character }),
        None => json!({ "triggerKind": 1 }),
    };

    let result = resolved
        .server
        .peer
        .request(
            "textDocument/completion",
            json!({
                "textDocument": { "uri": path_to_uri(file) },
                "position": position,
                "context": context,
            }),
            QUERY_TIMEOUT,
        )
        .await;

    let Ok(value) = result else {
        debug!("lsp: completion failed");
        return empty();
    };

    let (items, incomplete) = super::completion::render_result(&value);
    json!({
        "available": true,
        "items": items,
        "incomplete": incomplete,
        "triggerCharacters": caps.trigger_characters,
    })
}

pub(super) fn read_text(file: &Path, content: Option<&str>) -> String {
    match content {
        Some(text) => text.to_string(),
        None => std::fs::read_to_string(file).unwrap_or_default(),
    }
}

/// Shut a pool down on process exit, bounded so it cannot hang the exit path.
pub async fn shutdown(pool: &Arc<LspPool>) {
    let _ = tokio::time::timeout(Duration::from_secs(5), pool.shutdown_all()).await;
}
