//! Editor LSP API helpers — diagnostics, hover, definition, format.

use serde::Serialize;

use super::client::{api_fetch, api_post, ApiError};
use crate::types::api::*;

/// Fetch LSP diagnostics for a file.
pub async fn lsp_diagnostics(file: &str) -> Result<EditorLspDiagnosticsResponse, ApiError> {
    let encoded = js_sys::encode_uri_component(file);
    api_fetch(&format!("/editor/lsp/diagnostics?file={}", encoded)).await
}

/// Fetch LSP hover info.
pub async fn lsp_hover(file: &str, line: u32, col: u32) -> Result<EditorHoverResponse, ApiError> {
    let encoded = js_sys::encode_uri_component(file);
    api_fetch(&format!("/editor/lsp/hover?file={}&line={}&col={}", encoded, line, col)).await
}

/// Fetch LSP definition location.
pub async fn lsp_definition(file: &str, line: u32, col: u32) -> Result<EditorDefinitionResponse, ApiError> {
    let encoded = js_sys::encode_uri_component(file);
    api_fetch(&format!("/editor/lsp/definition?file={}&line={}&col={}", encoded, line, col)).await
}

#[derive(Serialize)]
struct FormatBody<'a> {
    file: &'a str,
    content: &'a str,
}

/// Format a file via LSP.
pub async fn lsp_format(file: &str, content: &str) -> Result<EditorFormatResponse, ApiError> {
    api_post("/editor/lsp/format", &FormatBody { file, content }).await
}
