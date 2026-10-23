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
        "annotations": annotations(name),
    })
}

fn annotations(name: &str) -> Value {
    let read_only = matches!(
        name,
        "glob" | "grep" | "list_dir" | "read_file" | "scheduler_list" | "wait" | "web_fetch"
    );
    let destructive = matches!(
        name,
        "bash" | "edit_file" | "scheduler_delete" | "scheduler_update" | "write_file"
    );
    let idempotent = read_only || matches!(name, "scheduler_delete" | "scheduler_update");
    let open_world = matches!(name, "bash" | "web_fetch");

    json!({
        "readOnlyHint": read_only,
        "destructiveHint": destructive,
        "idempotentHint": idempotent,
        "openWorldHint": open_world,
    })
}

#[cfg(test)]
mod tests {
    use super::{object, tool};
    use serde_json::{json, Value};

    fn definition(name: &str) -> Value {
        tool(
            name,
            "Test tool.",
            object(Vec::new(), &[]),
            object(Vec::new(), &[]),
        )
    }

    #[test]
    fn tool_definitions_classify_reads_and_mutations_for_clients() {
        let read = definition("read_file");
        assert_eq!(read["annotations"]["readOnlyHint"], json!(true));
        assert_eq!(read["annotations"]["destructiveHint"], json!(false));

        let web = definition("web_fetch");
        assert_eq!(web["annotations"]["readOnlyHint"], json!(true));
        assert_eq!(web["annotations"]["openWorldHint"], json!(true));

        let write = definition("write_file");
        assert_eq!(write["annotations"]["readOnlyHint"], json!(false));
        assert_eq!(write["annotations"]["destructiveHint"], json!(true));

        let shell = definition("bash");
        assert_eq!(shell["annotations"]["readOnlyHint"], json!(false));
        assert_eq!(shell["annotations"]["openWorldHint"], json!(true));
    }
}
