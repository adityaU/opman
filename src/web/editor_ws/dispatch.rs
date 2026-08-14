//! Op → language server call.
//!
//! Every arm resolves the path through the same project sandbox the REST
//! handlers use and then calls the same `crate::lsp` function, so the two front
//! doors cannot answer differently. The channel adds multiplexing and
//! cancellation; it does not add behaviour.

use serde_json::{json, Value};

use crate::web::handlers::editor_target;
use crate::web::types::{EditorLspQuery, ServerState};

use super::protocol::Op;

/// A payload that could not be understood is the caller's error, not a reason
/// to drop the connection — the id still gets an answer.
pub fn parse_query(payload: Value) -> Result<EditorLspQuery, String> {
    serde_json::from_value(payload).map_err(|error| format!("bad payload: {error}"))
}

pub async fn run(state: &ServerState, op: Op, payload: Value) -> Result<Value, String> {
    let query = parse_query(payload)?;
    let (file, project_dir) = editor_target(state, &query.path)
        .await
        .map_err(|error| error.to_string())?;
    let line = query.line.unwrap_or(1);
    let col = query.col.unwrap_or(1);
    let content = query.content.as_deref();

    let value = match op {
        Op::Hover => crate::lsp::api::hover(&state.lsp, &file, &project_dir, line, col, content).await,
        Op::Diagnostics => crate::lsp::api::diagnostics(&state.lsp, &file, &project_dir, content).await,
        Op::Completion => {
            crate::lsp::api::completion(
                &state.lsp, &file, &project_dir, line, col, content, query.trigger.as_deref(),
            )
            .await
        }
        Op::Goto => {
            let kind = query
                .goto
                .as_deref()
                .and_then(crate::lsp::api_edit::Goto::parse)
                .unwrap_or(crate::lsp::api_edit::Goto::Definition);
            crate::lsp::api_edit::goto(&state.lsp, kind, &file, &project_dir, line, col, content).await
        }
        Op::References => {
            crate::lsp::api_refactor::references(&state.lsp, &file, &project_dir, line, col, content).await
        }
        Op::Rename => {
            crate::lsp::api_refactor::rename(
                &state.lsp, &file, &project_dir, line, col,
                query.new_name.as_deref().unwrap_or_default(), content,
            )
            .await
        }
        Op::Format => crate::lsp::api_edit::format(&state.lsp, &file, &project_dir, content).await,
        // The file-manager ops are answered by `files`, and `cancel` never
        // reaches here — the session takes it before dispatch.
        Op::Browse | Op::Read | Op::Write | Op::CreateFile | Op::CreateDir | Op::Delete | Op::Move => {
            return Err(format!("{op:?} is not a language-server op"));
        }
        Op::Cancel => return Ok(json!(null)),
    };
    Ok(value)
}
