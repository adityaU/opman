use serde_json::json;

use super::*;

fn full(schema: serde_json::Value) -> String {
    type_of(&schema, Style::Full)
}

fn compact(schema: serde_json::Value) -> String {
    type_of(&schema, Style::Compact)
}

#[test]
fn primitives() {
    assert_eq!(full(json!({"type": "string"})), "string");
    assert_eq!(full(json!({"type": "integer"})), "number");
    assert_eq!(full(json!({"type": "number"})), "number");
    assert_eq!(full(json!({"type": "boolean"})), "boolean");
    assert_eq!(full(json!({"type": "null"})), "null");
}

#[test]
fn empty_or_odd_schema_is_unknown() {
    assert_eq!(full(json!({})), "unknown");
    assert_eq!(full(json!(null)), "unknown");
    assert_eq!(full(json!(true)), "unknown");
    assert_eq!(full(json!(false)), "never");
    assert_eq!(full(json!({"type": "weird"})), "unknown");
}

#[test]
fn object_marks_optional_fields_and_keeps_comments() {
    let text = full(json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File to read."},
            "limit": {"type": "integer", "default": 10}
        },
        "required": ["path"]
    }));
    assert_eq!(
        text,
        "{\n  /** File to read. */\n  path: string;\n  /** default: 10 */\n  limit?: number;\n}"
    );
}

#[test]
fn compact_object_is_one_line_without_comments() {
    let text = compact(json!({
        "type": "object",
        "properties": {"a": {"type": "string", "description": "x"}, "b": {"type": "boolean"}},
        "required": ["a"]
    }));
    assert_eq!(text, "{ a: string; b?: boolean }");
}

#[test]
fn nested_objects_indent_once_per_level() {
    let text = full(json!({
        "type": "object",
        "properties": {"outer": {"type": "object", "properties": {"inner": {"type": "string"}}}}
    }));
    assert_eq!(text, "{\n  outer?: {\n    inner?: string;\n  };\n}");
}

#[test]
fn awkward_field_names_are_quoted() {
    let text = compact(json!({"type": "object", "properties": {"a-b": {"type": "string"}}}));
    assert_eq!(text, "{ \"a-b\"?: string }");
}

#[test]
fn objects_without_fields() {
    assert_eq!(full(json!({"type": "object"})), "Record<string, unknown>");
    assert_eq!(
        full(json!({"type": "object", "additionalProperties": {"type": "number"}})),
        "Record<string, number>"
    );
    assert_eq!(
        full(json!({"type": "object", "additionalProperties": false})),
        "{}"
    );
}

#[test]
fn arrays_and_parenthesised_unions() {
    assert_eq!(
        full(json!({"type": "array", "items": {"type": "string"}})),
        "string[]"
    );
    assert_eq!(
        full(json!({"type": "array", "items": {"enum": ["a", "b"]}})),
        "(\"a\" | \"b\")[]"
    );
    assert_eq!(full(json!({"type": "array"})), "unknown[]");
    assert_eq!(
        full(
            json!({"type": "array", "items": {"type": "object", "properties": {"a": {"type": "string"}}}})
        ),
        "{\n  a?: string;\n}[]"
    );
}

#[test]
fn tuples() {
    let text = full(json!({"type": "array", "items": [{"type": "string"}, {"type": "number"}]}));
    assert_eq!(text, "[string, number]");
    let text = full(json!({"type": "array", "prefixItems": [{"type": "boolean"}]}));
    assert_eq!(text, "[boolean]");
}

#[test]
fn enums_and_consts() {
    assert_eq!(
        full(json!({"enum": ["a", 2, true, null]})),
        "\"a\" | 2 | true | null"
    );
    assert_eq!(full(json!({"const": "fixed"})), "\"fixed\"");
}

#[test]
fn huge_enum_falls_back_to_base_type() {
    let many: Vec<String> = (0..40).map(|n| format!("v{n}")).collect();
    assert_eq!(full(json!({"type": "string", "enum": many})), "string");
}

#[test]
fn unions_and_intersections() {
    assert_eq!(
        full(json!({"anyOf": [{"type": "string"}, {"type": "number"}]})),
        "string | number"
    );
    assert_eq!(
        full(json!({"oneOf": [{"type": "string"}, {"type": "string"}]})),
        "string"
    );
    assert_eq!(
        full(json!({"allOf": [{"type": "string"}, {"type": "number"}]})),
        "string & number"
    );
    assert_eq!(full(json!({"type": ["string", "null"]})), "string | null");
    assert_eq!(full(json!({"anyOf": [{"type": "string"}, {}]})), "unknown");
}

#[test]
fn nullable_flag_adds_null() {
    assert_eq!(
        full(json!({"type": "string", "nullable": true})),
        "string | null"
    );
}

#[test]
fn local_refs_resolve_and_cycles_stop() {
    let schema = json!({
        "type": "object",
        "properties": {"a": {"$ref": "#/$defs/Id"}},
        "$defs": {"Id": {"type": "string"}}
    });
    assert_eq!(compact(schema), "{ a?: string }");

    let cyclic = json!({
        "$defs": {"Node": {"type": "object", "properties": {"next": {"$ref": "#/$defs/Node"}}}},
        "$ref": "#/$defs/Node"
    });
    let text = compact(cyclic);
    assert!(text.contains("unknown"), "{text}");
    assert!(text.len() < 400);
    assert_eq!(full(json!({"$ref": "https://example.com/x"})), "unknown");
    assert_eq!(full(json!({"$ref": "#/missing"})), "unknown");
}

#[test]
fn comment_cannot_close_itself() {
    let text = full(
        json!({"type": "object", "properties": {"a": {"type": "string", "description": "bad */ end"}}}),
    );
    assert!(text.contains("bad * / end"), "{text}");
}

#[test]
fn long_descriptions_are_cut() {
    let long = "word ".repeat(200);
    let text = full(
        json!({"type": "object", "properties": {"a": {"type": "string", "description": long}}}),
    );
    assert!(text.contains("...*/") || text.contains("... */"), "{text}");
    assert!(text.len() < 400);
}

#[test]
fn many_fields_are_summarised() {
    let props: serde_json::Map<String, serde_json::Value> = (0..70)
        .map(|n| (format!("f{n:02}"), json!({"type": "string"})))
        .collect();
    let text = full(json!({"type": "object", "properties": props}));
    assert!(text.contains("// ... 10 more fields"), "{text}");
}

fn tool(
    name: &str,
    description: Option<&str>,
    input: serde_json::Value,
    output: serde_json::Value,
) -> ToolDef {
    serde_json::from_value(json!({
        "name": name, "description": description, "inputSchema": input, "outputSchema": output
    }))
    .expect("tool")
}

#[test]
fn signature_has_comment_call_and_types() {
    let def = tool(
        "read_file",
        Some("Read a file."),
        json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        json!({"type": "object", "properties": {"text": {"type": "string"}}}),
    );
    let text = signature("opman_tools", &def, Style::Compact);
    assert_eq!(
        text,
        "/** Read a file. */\nawait tools.opman_tools.read_file(input: { path: string }): Promise<{ text?: string }>"
    );
}

#[test]
fn signature_without_required_makes_input_optional_and_output_unknown() {
    let def = tool("now", None, json!({"type": "object"}), json!(null));
    let text = signature("time", &def, Style::Full);
    assert_eq!(
        text,
        "await tools.time.now(input?: Record<string, unknown>): Promise<unknown>"
    );
}

#[test]
fn signature_uses_brackets_for_odd_tool_names() {
    let def = tool("get-thing", None, json!({"type": "object"}), json!(null));
    let text = signature("x", &def, Style::Full);
    assert!(text.starts_with("await tools.x[\"get-thing\"]("), "{text}");
}
