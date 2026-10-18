//! Names the model types: server namespaces, access paths, and "did you mean".

use std::collections::HashSet;

/// The namespace that always exists, for tool discovery.
pub const CODEMODE: &str = "$codemode";

/// A JavaScript-safe name for one server.
///
/// Only [`Namespace::for_server`] makes one, so every value is a valid identifier and
/// never collides with another namespace or with [`CODEMODE`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Namespace(Box<str>);

impl Namespace {
    /// Sanitise `server`, then add a numeric suffix while the result is already `taken`.
    pub fn for_server(server: &str, taken: &HashSet<Namespace>) -> Self {
        let base = sanitize(server);
        let mut candidate = base.clone();
        let mut n = 2_u32;
        while candidate == CODEMODE || taken.contains(&Namespace(candidate.clone().into())) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        Self(candidate.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn sanitize(server: &str) -> String {
    let mut out: String = server
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push_str("server");
    }
    if out.starts_with(|ch: char| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// Whether `text` can follow a `.` in JavaScript without brackets.
pub fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let start = first.is_ascii_alphabetic() || first == '_' || first == '$';
    start && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

/// `tools.ns.tool`, or `tools.ns["odd-name"]` when the tool name is not an identifier.
pub fn access_path(namespace: &str, tool: &str) -> String {
    if is_identifier(tool) {
        return format!("tools.{namespace}.{tool}");
    }
    format!("tools.{namespace}[{}]", serde_json::Value::from(tool))
}

/// Up to three of `names` that look like what the model meant by `wanted`.
pub fn closest<'a>(wanted: &str, names: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let wanted = wanted.to_lowercase();
    let reach = (wanted.chars().count() / 3).max(2);
    let mut scored: Vec<(usize, &str)> = names
        .filter_map(|name| {
            let lower = name.to_lowercase();
            let near = distance(&wanted, &lower);
            let contained =
                !wanted.is_empty() && (lower.contains(&wanted) || wanted.contains(&lower));
            (near <= reach || contained).then_some((if contained { 0 } else { near }, name))
        })
        .collect();
    scored.sort();
    scored.into_iter().take(3).map(|(_, name)| name).collect()
}

/// Levenshtein distance over characters, in two rows.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            current[j + 1] = (previous[j] + cost)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod names_tests;
