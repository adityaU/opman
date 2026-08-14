//! Definition and format — the two operations that move the reader somewhere
//! else, or move the file underneath them.
//!
//! Split from [`super::api`] to keep each file readable; they share its
//! `prepare` helper, which resolves the server and syncs the document.

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use tracing::debug;

use super::api::{prepare, read_text, unavailable};
use super::convert::{
    apply_text_edits, definition_targets, from_lsp_position, path_to_uri, uri_to_path,
};
use super::pool::LspPool;
use super::server::{FORMAT_TIMEOUT, QUERY_TIMEOUT};

// ── Definition ──────────────────────────────────────────

/// The four "take me to the symbol" queries. They differ only in the LSP method
/// and the capability that gates it, so they are one enum rather than four
/// near-identical functions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Goto {
    Definition,
    TypeDefinition,
    Implementation,
    Declaration,
}

impl Goto {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "definition" => Some(Self::Definition),
            "type-definition" | "typeDefinition" => Some(Self::TypeDefinition),
            "implementation" => Some(Self::Implementation),
            "declaration" => Some(Self::Declaration),
            _ => None,
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::Definition => "textDocument/definition",
            Self::TypeDefinition => "textDocument/typeDefinition",
            Self::Implementation => "textDocument/implementation",
            Self::Declaration => "textDocument/declaration",
        }
    }

    fn supported_by(self, caps: &super::server::ServerCaps) -> bool {
        match self {
            Self::Definition => caps.definition,
            Self::TypeDefinition => caps.type_definition,
            Self::Implementation => caps.implementation,
            Self::Declaration => caps.declaration,
        }
    }
}

pub async fn definition(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    line: i64,
    col: i64,
    content: Option<&str>,
) -> Value {
    goto(pool, Goto::Definition, file, project_dir, line, col, content).await
}

#[allow(clippy::too_many_arguments)]
pub async fn goto(
    pool: &Arc<LspPool>,
    kind: Goto,
    file: &Path,
    project_dir: &Path,
    line: i64,
    col: i64,
    content: Option<&str>,
) -> Value {
    let Some((resolved, caps)) = prepare(pool, file, project_dir, content).await else {
        return unavailable(&[("locations", json!([]))]);
    };
    if !kind.supported_by(&caps) {
        return unavailable(&[("locations", json!([]))]);
    }
    let text = read_text(file, content);
    let position = super::convert::to_lsp_position(&text, line, col, caps.encoding);

    let result = resolved
        .server
        .peer
        .request(
            kind.method(),
            json!({ "textDocument": { "uri": path_to_uri(file) }, "position": position }),
            QUERY_TIMEOUT,
        )
        .await;

    let Ok(value) = result else {
        debug!(?kind, "lsp: goto failed");
        return unavailable(&[("locations", json!([]))]);
    };

    let locations: Vec<Value> = definition_targets(&value)
        .into_iter()
        .filter_map(|(uri, start)| {
            let target = uri_to_path(&uri)?;
            // Positions come back relative to the *target* file, so its own
            // text is what the column must be measured against.
            let target_text = std::fs::read_to_string(&target).unwrap_or_default();
            let (lnum, col) = from_lsp_position(&target_text, &start, caps.encoding);
            Some(json!({ "file": target.to_string_lossy(), "lnum": lnum, "col": col }))
        })
        .collect();

    json!({ "available": true, "locations": locations })
}

// ── Format ──────────────────────────────────────────────

pub async fn format(
    pool: &Arc<LspPool>,
    file: &Path,
    project_dir: &Path,
    content: Option<&str>,
) -> Value {
    let original = read_text(file, content);
    let Some((resolved, caps)) = prepare(pool, file, project_dir, content).await else {
        return unavailable(&[("formatted", json!(false)), ("content", json!(original))]);
    };
    if !caps.formatting {
        return unavailable(&[("formatted", json!(false)), ("content", json!(original))]);
    }

    let result = resolved
        .server
        .peer
        .request(
            "textDocument/formatting",
            json!({
                "textDocument": { "uri": path_to_uri(file) },
                "options": { "tabSize": 4, "insertSpaces": true },
            }),
            FORMAT_TIMEOUT,
        )
        .await;

    let edits = match result {
        Ok(Value::Array(edits)) if !edits.is_empty() => edits,
        Ok(_) => return json!({ "available": true, "formatted": false, "content": original }),
        Err(e) => {
            debug!("lsp: format failed: {e}");
            return unavailable(&[("formatted", json!(false)), ("content", json!(original))]);
        }
    };

    let Some(formatted) = apply_text_edits(&original, &edits, caps.encoding) else {
        debug!("lsp: format edits did not apply cleanly");
        return json!({ "available": true, "formatted": false, "content": original });
    };
    if formatted == original {
        return json!({ "available": true, "formatted": false, "content": original });
    }

    if let Err(e) = std::fs::write(file, &formatted) {
        debug!("lsp: could not write formatted file: {e}");
        return json!({ "available": true, "formatted": false, "content": original });
    }
    // Re-sync so the server's copy matches what we just wrote, then tell it the
    // file was saved — many servers only recompute diagnostics on save.
    let _ = resolved.server.docs.sync(
        &resolved.server.peer,
        file,
        resolved.spec.language,
        Some(&formatted),
    );
    resolved
        .server
        .docs
        .notify_saved(&resolved.server.peer, file);

    json!({ "available": true, "formatted": true, "content": formatted })
}
