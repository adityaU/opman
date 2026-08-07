//! Frames the server sends us.
//!
//! Two of these are not optional. `publishDiagnostics` is the only way
//! diagnostics ever arrive. And `workspace/configuration` is a *request*: we
//! declared the capability, so a server that asks and gets no answer will sit
//! there waiting — rust-analyzer in particular blocks part of its startup on
//! it. Answering nulls means "no settings from us, use your defaults", which is
//! exactly right and infinitely better than the silence.

use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};
use tracing::debug;

use super::diags::DiagStore;
use super::peer::Handler;

pub struct ServerHandler {
    pub diags: Arc<DiagStore>,
}

impl Handler for ServerHandler {
    fn request(&self, method: &str, params: &Value) -> Result<Value> {
        match method {
            // One entry per requested item, all defaults.
            "workspace/configuration" => {
                let count = params
                    .get("items")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0);
                Ok(Value::Array(vec![Value::Null; count]))
            }
            // We register nothing dynamically, but must acknowledge.
            "client/registerCapability" | "client/unregisterCapability" => Ok(Value::Null),
            // Progress tokens are accepted so servers report indexing; we do
            // not surface it yet, but refusing makes some servers go quiet.
            "window/workDoneProgress/create" => Ok(Value::Null),
            // No UI to show a message request in — decline by choosing nothing.
            "window/showMessageRequest" => Ok(Value::Null),
            "workspace/applyEdit" => Ok(json!({ "applied": false })),
            other => {
                debug!(method = other, "lsp: unhandled server request");
                Ok(Value::Null)
            }
        }
    }

    fn notify(&self, method: &str, params: Value) {
        match method {
            "textDocument/publishDiagnostics" => {
                let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                    return;
                };
                let diagnostics = params
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                self.diags.publish(uri.to_string(), diagnostics);
            }
            "window/logMessage" | "window/showMessage" | "$/progress" | "telemetry/event" => {}
            other => debug!(method = other, "lsp: unhandled server notification"),
        }
    }
}
