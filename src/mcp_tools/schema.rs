//! Small helpers to build JSON Schema objects, so the tool catalogs stay short.

use serde_json::{json, Map, Value};

pub fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

pub fn integer(description: &str) -> Value {
    json!({ "type": "integer", "minimum": 0, "description": description })
}

pub fn boolean(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

pub fn choice(values: &[&str], description: &str) -> Value {
    json!({ "type": "string", "enum": values, "description": description })
}

pub fn list_of(item: Value, description: &str) -> Value {
    json!({ "type": "array", "items": item, "description": description })
}

/// An object schema. Properties keep the order given.
pub fn object(properties: Vec<(&str, Value)>, required: &[&str]) -> Value {
    let properties: Map<String, Value> = properties
        .into_iter()
        .map(|(name, schema)| (name.to_string(), schema))
        .collect();
    json!({ "type": "object", "properties": properties, "required": required })
}

/// A complete tool entry for `tools/list`.
pub fn tool(name: &str, description: &str, input: Value, output: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input,
        "outputSchema": output,
    })
}
