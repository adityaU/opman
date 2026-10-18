//! JSON Schema to TypeScript-style types, for the `execute` tool's description.
//!
//! The model writes JavaScript against `tools.<server>.<tool>(input)`. What it needs
//! from each tool is the shape of `input` and of the result, as a type it already reads
//! fluently. This is a renderer, not a validator: anything it cannot express becomes
//! `unknown`, which is always safe for the model to narrow.

mod comment;
mod object;
mod ts;

pub use ts::{type_of, Style};

use serde_json::Value;

use crate::mcp_probe::ToolDef;

use super::names::access_path;

/// One tool as the model should see it: a comment, then the exact call.
pub fn signature(namespace: &str, tool: &ToolDef, style: Style) -> String {
    let mut out = String::new();
    if let Some(text) = tool.description.as_deref() {
        out.push_str(&comment::block(text, comment::TOOL_LIMIT));
        out.push('\n');
    }
    let input = type_of(&tool.input_schema, style);
    // A call with nothing required may omit the argument entirely.
    let optional = if has_required(&tool.input_schema) {
        ""
    } else {
        "?"
    };
    let output = match &tool.output_schema {
        Value::Null => "unknown".to_string(),
        schema => type_of(schema, style),
    };
    out.push_str(&format!(
        "await {}(input{optional}: {input}): Promise<{output}>",
        access_path(namespace, &tool.name)
    ));
    out
}

fn has_required(schema: &Value) -> bool {
    schema
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| !required.is_empty())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
