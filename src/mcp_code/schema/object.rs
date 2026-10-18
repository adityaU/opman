//! Object schemas: fields, requiredness, comments, and the `Record` forms.

use serde_json::{Map, Value};

use super::comment;
use super::ts::{Renderer, Style};
use crate::mcp_code::names::is_identifier;

/// Fields past this are summarised, so one sprawling schema cannot eat the budget.
const MAX_FIELDS: usize = 60;

pub(super) fn render(
    renderer: &Renderer<'_>,
    map: &Map<String, Value>,
    depth: usize,
    indent: usize,
) -> String {
    let fields = map.get("properties").and_then(Value::as_object);
    let Some(fields) = fields.filter(|fields| !fields.is_empty()) else {
        return open_object(renderer, map, depth, indent);
    };
    let required: Vec<&str> = map
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let required_fields = required
        .iter()
        .filter_map(|name| fields.get_key_value(*name));
    let optional_fields = fields
        .iter()
        .filter(|(name, _)| !required.contains(&name.as_str()));
    let lines: Vec<Field> = required_fields
        .chain(optional_fields)
        .take(MAX_FIELDS)
        .map(|(name, schema)| {
            field(
                renderer,
                name,
                schema,
                required.contains(&name.as_str()),
                depth,
                indent,
            )
        })
        .collect();
    let hidden = fields.len().saturating_sub(MAX_FIELDS);
    wrap(&lines, hidden, indent, renderer.style)
}

/// One field: an optional comment line, then the declaration. A nested object's lines
/// are already indented for their depth, so only these two heads get a prefix.
struct Field {
    note: String,
    decl: String,
}

/// An object with no named fields: a string-keyed map, or any object at all.
fn open_object(
    renderer: &Renderer<'_>,
    map: &Map<String, Value>,
    depth: usize,
    indent: usize,
) -> String {
    match map.get("additionalProperties") {
        Some(Value::Bool(false)) => "{}".into(),
        Some(extra @ Value::Object(_)) => {
            format!(
                "Record<string, {}>",
                renderer.render(extra, depth + 1, indent)
            )
        }
        _ => "Record<string, unknown>".into(),
    }
}

fn field(
    renderer: &Renderer<'_>,
    name: &str,
    schema: &Value,
    required: bool,
    depth: usize,
    indent: usize,
) -> Field {
    let key = if is_identifier(name) {
        name.to_string()
    } else {
        Value::from(name).to_string()
    };
    let mark = if required { "" } else { "?" };
    let ty = renderer.render(schema, depth + 1, indent + 1);
    let note = match renderer.style {
        Style::Full => note_for(schema),
        Style::Compact => String::new(),
    };
    Field {
        note,
        decl: format!("{key}{mark}: {ty}"),
    }
}

fn note_for(schema: &Value) -> String {
    let text = schema
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    let default = schema.get("default").filter(|value| !value.is_null());
    match (text.is_empty(), default) {
        (true, None) => String::new(),
        (true, Some(value)) => comment::line(&format!("default: {value}"), comment::FIELD_LIMIT),
        (false, None) => comment::line(text, comment::FIELD_LIMIT),
        (false, Some(value)) => {
            comment::line(&format!("{text} (default: {value})"), comment::FIELD_LIMIT)
        }
    }
}

fn wrap(lines: &[Field], hidden: usize, indent: usize, style: Style) -> String {
    match style {
        // A comment cannot sit inside a one-line object without hiding the fields
        // after it, so compact drops them.
        Style::Compact => {
            let decls: Vec<&str> = lines.iter().map(|line| line.decl.as_str()).collect();
            let mut body = decls.join("; ");
            if hidden > 0 {
                body.push_str(&format!("; /* {hidden} more */"));
            }
            format!("{{ {body} }}")
        }
        Style::Full => {
            let inner = "  ".repeat(indent + 1);
            let mut out = String::from("{\n");
            for line in lines {
                if !line.note.is_empty() {
                    out.push_str(&format!("{inner}{}\n", line.note));
                }
                out.push_str(&format!("{inner}{};\n", line.decl));
            }
            if hidden > 0 {
                out.push_str(&format!("{inner}// ... {hidden} more fields\n"));
            }
            out.push_str(&"  ".repeat(indent));
            out.push('}');
            out
        }
    }
}
