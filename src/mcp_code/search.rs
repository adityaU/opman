//! `tools.$codemode.search`: find tools by what they do.

use serde_json::{json, Value};

use super::catalog::Catalog;
use super::names::access_path;
use super::schema::{signature, Style};

/// Hits returned per search. Enough to choose from, few enough to read.
const MAX_HITS: usize = 8;

/// Score every tool against the query words and return the best as
/// `[{ path, description, signature }]`.
pub fn search(catalog: &Catalog, query: &str) -> Value {
    let words: Vec<String> = query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut hits: Vec<(usize, &str, &crate::mcp_probe::ToolDef)> = Vec::new();
    for group in &catalog.groups {
        for tool in group.tools.iter() {
            let score = score(group.namespace.as_str(), tool, &words);
            if score > 0 {
                hits.push((score, group.namespace.as_str(), tool));
            }
        }
    }
    // Best first; the path breaks ties so results are stable between calls.
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.2.name.cmp(&b.2.name)));
    let found: Vec<Value> = hits
        .into_iter()
        .take(MAX_HITS)
        .map(|(_, namespace, tool)| {
            json!({
                "path": access_path(namespace, &tool.name),
                "description": tool.description.as_deref().unwrap_or(""),
                "signature": signature(namespace, tool, Style::Full),
            })
        })
        .collect();
    Value::Array(found)
}

fn score(namespace: &str, tool: &crate::mcp_probe::ToolDef, words: &[String]) -> usize {
    let name = tool.name.to_lowercase();
    let namespace = namespace.to_lowercase();
    let description = tool.description.as_deref().unwrap_or("").to_lowercase();
    words
        .iter()
        .map(|word| {
            let mut points = 0;
            if name.contains(word.as_str()) {
                points += 3;
            }
            if namespace.contains(word.as_str()) {
                points += 2;
            }
            if description.contains(word.as_str()) {
                points += 1;
            }
            points
        })
        .sum()
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod search_tests;
