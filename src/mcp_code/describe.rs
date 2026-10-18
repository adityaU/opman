//! The `execute` tool's description: how to use it, and what it can call.
//!
//! The description is the model's whole manual, so it is generated from the live catalog.
//! A big catalog would swamp the context window, so there are three levels, tried in
//! order: every tool in full, every tool compact, and finally only the server names plus
//! a pointer to `tools.$codemode.search`.

use std::fmt::Write as _;

use super::catalog::Catalog;
use super::names::CODEMODE;
use super::schema::{signature, Style};

/// Characters the tool list may take before the description falls back a level.
pub const DEFAULT_BUDGET: usize = 14_000;

const INTRO: &str = "Run JavaScript that calls tools. `code` is the body of an async function: \
use `await`, `return` the value you want back, and `console.log` for notes. The sandbox has \
no filesystem, network, or process access of its own; everything goes through `tools`.";

const WORKFLOW: &str = "## Workflow\n\
1. Look at the tools listed below, or call `tools.$codemode.search({ query })` to find some.\n\
2. Write code that calls them with `await tools.<namespace>.<tool>(input)`.\n\
3. Return only what you need. The returned value is printed as JSON (strings as-is).";

const RULES: &str = "## Rules\n\
- Only the tools listed here exist. Do not invent others.\n\
- Filter and aggregate in code; return just the fields you need, not whole payloads.\n\
- Run independent calls in parallel with `Promise.all`.\n\
- `Object.keys(tools)` lists the namespaces; `Object.keys(tools.<namespace>)` lists its tools.\n\
- A failed tool call throws an Error holding the tool's message; catch it if you can recover.\n\
- Results are `unknown` unless a type is shown: check a value before reading its fields.\n\
- Images and files a tool returns are attached to the result, not inlined.";

/// How the tool list was rendered, so callers and tests can tell the levels apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listing {
    Full,
    Compact,
    SearchOnly,
}

pub fn build(catalog: &Catalog, budget: usize) -> (String, Listing) {
    let mut out = String::with_capacity(budget.min(24_000));
    let _ = write!(
        out,
        "{INTRO}\n\n{WORKFLOW}\n\n{RULES}\n\n## Available tools\n\n"
    );

    let (listing, body) = list(catalog, budget);
    out.push_str(&body);
    if !catalog.unavailable.is_empty() {
        out.push_str("\nUnavailable right now:\n");
        for missing in &catalog.unavailable {
            let _ = writeln!(out, "- {}: {}", missing.server, missing.reason);
        }
    }
    (out, listing)
}

fn list(catalog: &Catalog, budget: usize) -> (Listing, String) {
    if catalog.groups.is_empty() {
        return (Listing::Full, "(no tools are available)\n".into());
    }
    for (style, listing) in [
        (Style::Full, Listing::Full),
        (Style::Compact, Listing::Compact),
    ] {
        let body = render(catalog, style);
        if body.len() <= budget {
            return (listing, body);
        }
    }
    (Listing::SearchOnly, search_only(catalog))
}

fn render(catalog: &Catalog, style: Style) -> String {
    let mut out = String::new();
    for group in &catalog.groups {
        let _ = writeln!(out, "### {}\n", group.namespace.as_str());
        for tool in group.tools.iter() {
            let _ = write!(
                out,
                "{}\n\n",
                signature(group.namespace.as_str(), tool, style)
            );
        }
    }
    out
}

fn search_only(catalog: &Catalog) -> String {
    let mut out = format!(
        "There are {} tools, too many to list. Find the ones you need first:\n\
         `await tools.{CODEMODE}.search({{ query: \"what you want to do\" }})` returns matching \
         tool paths with their exact signatures.\n\nNamespaces:\n",
        catalog.tool_count()
    );
    for group in &catalog.groups {
        let _ = writeln!(
            out,
            "- {} ({} tools)",
            group.namespace.as_str(),
            group.tools.len()
        );
    }
    out
}

#[cfg(test)]
#[path = "describe_tests.rs"]
mod describe_tests;
