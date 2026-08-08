//! Turning a server's completion list into something the editor can render.
//!
//! Three shapes have to be reconciled. A server may answer with a bare array or
//! a `CompletionList`; an item may carry its text in `insertText`, in a
//! `textEdit`, or only in `label`; and that text may be a snippet in LSP's
//! syntax, which is not CodeMirror's. Getting any of them wrong produces
//! completions that insert the wrong string — worse than none at all.

use serde_json::{json, Value};

/// Cap what we send. rust-analyzer will happily return two thousand items for
/// an empty prefix, and the editor filters client-side anyway.
const MAX_ITEMS: usize = 300;

/// `CompletionItemKind` → a short name the editor maps to an icon. The numbers
/// are fixed by the protocol.
fn kind_name(kind: i64) -> &'static str {
    match kind {
        1 => "text",
        2 => "method",
        3 => "function",
        4 => "constructor",
        5 => "field",
        6 => "variable",
        7 => "class",
        8 => "interface",
        9 => "module",
        10 => "property",
        11 => "unit",
        12 => "value",
        13 => "enum",
        14 => "keyword",
        15 => "snippet",
        16 => "color",
        17 => "file",
        18 => "reference",
        19 => "folder",
        20 => "enumMember",
        21 => "constant",
        22 => "struct",
        23 => "event",
        24 => "operator",
        25 => "typeParameter",
        _ => "text",
    }
}

/// Flatten `documentation`, which is either a string or a `MarkupContent`.
fn documentation(item: &Value) -> Option<String> {
    let doc = item.get("documentation")?;
    let text = match doc {
        Value::String(s) => s.clone(),
        Value::Object(map) => map.get("value").and_then(Value::as_str)?.to_string(),
        _ => return None,
    };
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// The text an item actually inserts, and whether it is a snippet.
///
/// `textEdit` wins when present — it is the only form that can replace a range
/// wider than the word being typed, which is how servers rewrite `foo.bar` into
/// `bar(&foo)` and similar.
fn insertion(item: &Value) -> (String, bool) {
    let snippet = item.get("insertTextFormat").and_then(Value::as_i64) == Some(2);

    if let Some(edit) = item.get("textEdit") {
        if let Some(text) = edit.get("newText").and_then(Value::as_str) {
            return (text.to_string(), snippet);
        }
    }
    if let Some(text) = item.get("insertText").and_then(Value::as_str) {
        return (text.to_string(), snippet);
    }
    let label = item
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or_default();
    (label.to_string(), false)
}

/// Rewrite an LSP snippet into CodeMirror's template syntax.
///
/// LSP writes `${1:name}`, `${1}`, `$1` and `$0`; CodeMirror writes `${name}`
/// for a field and `${}` for an empty one, and has no notion of tab order
/// beyond document order — which for practical completions is the same order.
/// Escapes (`\$`, `\}`, `\\`) must survive intact.
pub fn snippet_to_codemirror(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // An escaped character passes through as itself.
            if let Some(next) = chars.next() {
                if !matches!(next, '$' | '}' | '\\') {
                    out.push('\\');
                }
                out.push(next);
            }
            continue;
        }
        if ch != '$' {
            // A bare `{` or `}` would read as a CodeMirror field boundary.
            if ch == '#' && chars.peek() == Some(&'{') {
                out.push_str("\\#");
                continue;
            }
            out.push(ch);
            continue;
        }

        match chars.peek() {
            // `${...}` — either `${1}` or `${1:default}`.
            Some('{') => {
                chars.next();
                let mut body = String::new();
                let mut depth = 1;
                for inner in chars.by_ref() {
                    if inner == '{' {
                        depth += 1;
                    }
                    if inner == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    body.push(inner);
                }
                let default = body.split_once(':').map(|(_, rest)| rest).unwrap_or("");
                out.push_str("${");
                out.push_str(default);
                out.push('}');
            }
            // `$1` / `$0` — a field with no default.
            Some(c) if c.is_ascii_digit() => {
                while matches!(chars.peek(), Some(c) if c.is_ascii_digit()) {
                    chars.next();
                }
                out.push_str("${}");
            }
            // A literal dollar sign.
            _ => out.push('$'),
        }
    }
    out
}

/// Normalise one item into the editor's shape.
fn render_item(item: &Value) -> Option<Value> {
    let label = item.get("label").and_then(Value::as_str)?;
    let (text, is_snippet) = insertion(item);
    let kind = kind_name(item.get("kind").and_then(Value::as_i64).unwrap_or(1));

    // `labelDetails` carries the signature suffix that makes two same-named
    // completions distinguishable, e.g. `push(&mut self, value: T)`.
    let detail = item
        .get("labelDetails")
        .and_then(|d| d.get("detail"))
        .and_then(Value::as_str)
        .or_else(|| item.get("detail").and_then(Value::as_str))
        .unwrap_or_default();

    Some(json!({
        "label": label,
        "kind": kind,
        "detail": detail,
        "documentation": documentation(item),
        "insert": if is_snippet { snippet_to_codemirror(&text) } else { text },
        "snippet": is_snippet,
        // Servers rank far better than an alphabetical client sort — a leading
        // sortText is how rust-analyzer puts the field you actually want first.
        "sort": item.get("sortText").and_then(Value::as_str).unwrap_or(label),
        "filter": item.get("filterText").and_then(Value::as_str).unwrap_or(label),
        "preselect": item.get("preselect").and_then(Value::as_bool).unwrap_or(false),
        "deprecated": is_deprecated(item),
    }))
}

fn is_deprecated(item: &Value) -> bool {
    if item.get("deprecated").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    item.get("tags")
        .and_then(Value::as_array)
        .is_some_and(|tags| tags.iter().any(|tag| tag.as_i64() == Some(1)))
}

/// Normalise a whole `textDocument/completion` result.
///
/// Returns the items and whether the list is incomplete — when it is, the
/// editor must re-query as the user keeps typing instead of filtering what it
/// already has, or it will narrow down to nothing.
pub fn render_result(result: &Value) -> (Vec<Value>, bool) {
    let (items, incomplete) = match result {
        Value::Array(items) => (items.clone(), false),
        Value::Object(map) => (
            map.get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            map.get("isIncomplete")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        _ => (Vec::new(), false),
    };

    let rendered: Vec<Value> = items
        .iter()
        .take(MAX_ITEMS)
        .filter_map(render_item)
        .collect();
    (rendered, incomplete || items.len() > MAX_ITEMS)
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod completion_tests;
