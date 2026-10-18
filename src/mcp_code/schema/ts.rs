//! The type renderer proper.

use serde_json::{Map, Value};

use super::object;

/// How much detail to spend. `Compact` is the fallback when the full catalog would not
/// fit the description budget: no field comments, objects on one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Full,
    Compact,
}

/// Deeper than this renders as `unknown`: it bounds both output size and `$ref` cycles.
const MAX_DEPTH: usize = 6;
/// More enum members than this are rendered as the base type, not a giant union.
const MAX_LITERALS: usize = 24;

/// The TypeScript-style type of `schema`. A missing or empty schema is `unknown`.
pub fn type_of(schema: &Value, style: Style) -> String {
    let renderer = Renderer {
        root: schema,
        style,
    };
    renderer.render(schema, 0, 0)
}

pub(super) struct Renderer<'a> {
    pub(super) root: &'a Value,
    pub(super) style: Style,
}

impl Renderer<'_> {
    /// `indent` is the nesting level of the line this type starts on.
    pub(super) fn render(&self, schema: &Value, depth: usize, indent: usize) -> String {
        if depth > MAX_DEPTH {
            return "unknown".into();
        }
        let map = match schema {
            Value::Bool(false) => return "never".into(),
            Value::Object(map) => map,
            _ => return "unknown".into(),
        };
        let rendered = self.shape(map, depth, indent);
        let nullable = map.get("nullable").and_then(Value::as_bool) == Some(true);
        if nullable && rendered != "unknown" && !rendered.contains("null") {
            return format!("{rendered} | null");
        }
        rendered
    }

    fn shape(&self, map: &Map<String, Value>, depth: usize, indent: usize) -> String {
        if let Some(target) = map.get("$ref").and_then(Value::as_str) {
            return match self.resolve(target) {
                Some(found) => self.render(found, depth + 1, indent),
                None => "unknown".into(),
            };
        }
        if let Some(value) = map.get("const") {
            return literal(value);
        }
        if let Some(Value::Array(values)) = map.get("enum") {
            return self.enumeration(map, values, depth, indent);
        }
        for key in ["anyOf", "oneOf"] {
            if let Some(Value::Array(options)) = map.get(key) {
                return self.join(options, " | ", depth, indent);
            }
        }
        if let Some(Value::Array(parts)) = map.get("allOf") {
            return self.join(parts, " & ", depth, indent);
        }
        match map.get("type") {
            Some(Value::String(kind)) => self.by_type(kind, map, depth, indent),
            Some(Value::Array(kinds)) => {
                let parts: Vec<String> = kinds
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|kind| self.by_type(kind, map, depth, indent))
                    .collect();
                union(parts, " | ")
            }
            _ if map.contains_key("properties") => self.by_type("object", map, depth, indent),
            _ if map.contains_key("items") => self.by_type("array", map, depth, indent),
            _ => "unknown".into(),
        }
    }

    fn by_type(&self, kind: &str, map: &Map<String, Value>, depth: usize, indent: usize) -> String {
        match kind {
            "string" => "string".into(),
            "number" | "integer" => "number".into(),
            "boolean" => "boolean".into(),
            "null" => "null".into(),
            "array" => self.array(map, depth, indent),
            "object" => object::render(self, map, depth, indent),
            _ => "unknown".into(),
        }
    }

    fn array(&self, map: &Map<String, Value>, depth: usize, indent: usize) -> String {
        let tuple = map
            .get("prefixItems")
            .or_else(|| map.get("items").filter(|items| items.is_array()));
        if let Some(Value::Array(items)) = tuple {
            let parts: Vec<String> = items
                .iter()
                .map(|item| self.render(item, depth + 1, indent))
                .collect();
            return format!("[{}]", parts.join(", "));
        }
        let element = match map.get("items") {
            Some(items) => self.render(items, depth + 1, indent),
            None => "unknown".into(),
        };
        if has_top_level_operator(&element) {
            return format!("({element})[]");
        }
        format!("{element}[]")
    }

    fn enumeration(
        &self,
        map: &Map<String, Value>,
        values: &[Value],
        depth: usize,
        indent: usize,
    ) -> String {
        if values.len() > MAX_LITERALS {
            // A huge enum helps nobody; its base type says the same thing in one word.
            let mut plain = map.clone();
            plain.remove("enum");
            return self.shape(&plain, depth, indent);
        }
        union(values.iter().map(literal).collect(), " | ")
    }

    fn join(&self, options: &[Value], glue: &str, depth: usize, indent: usize) -> String {
        let parts = options
            .iter()
            .map(|option| self.render(option, depth + 1, indent))
            .collect();
        union(parts, glue)
    }

    /// Local `#/...` pointers only: following a remote reference would mean fetching.
    fn resolve(&self, target: &str) -> Option<&Value> {
        let pointer = target.strip_prefix('#')?;
        if pointer.is_empty() {
            return Some(self.root);
        }
        self.root.pointer(pointer)
    }
}

/// Deduplicate, drop `unknown` when something more precise exists, and join.
fn union(mut parts: Vec<String>, glue: &str) -> String {
    let mut seen: Vec<String> = Vec::with_capacity(parts.len());
    for part in parts.drain(..) {
        if !seen.contains(&part) {
            seen.push(part);
        }
    }
    if seen.is_empty() {
        return "unknown".into();
    }
    if seen.iter().any(|part| part == "unknown") {
        return "unknown".into();
    }
    seen.join(glue)
}

fn literal(value: &Value) -> String {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => value.to_string(),
        _ => "unknown".into(),
    }
}

/// True when `text` has a `|` or `&` outside every bracket and string, which is when an
/// array suffix needs parentheses.
fn has_top_level_operator(text: &str) -> bool {
    let (mut level, mut quoted, mut escaped) = (0_i32, false, false);
    for ch in text.chars() {
        if quoted {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '{' | '(' | '[' | '<' => level += 1,
            '}' | ')' | ']' | '>' => level -= 1,
            '|' | '&' if level == 0 => return true,
            _ => {}
        }
    }
    false
}
