//! The tool surface skills present to a runner.
//!
//! One zero-argument tool per skill, plus a permanent `skill_list`/`skill_load` pair.
//!
//! The pair alone does not get skills *used*: nothing in the model's context says they
//! exist, so discovering them costs a speculative call the model has no reason to make.
//! A tool per skill puts each name and description in context up front — around fifty
//! tokens each — while the body, commonly one to five thousand, stays deferred to the
//! call. Past [`SKILL_TOOL_LIMIT`] that trade stops paying, and the pair carries it.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{json, Value};

use super::name::SkillName;
use super::store::SkillStore;
use super::Skill;

/// Above this many skills, per-skill tools would crowd out the runner's own tools.
pub const SKILL_TOOL_LIMIT: usize = 64;

const PREFIX: &str = "skill_";

/// Map tool name to skill, for the whole set. Empty above the limit.
pub fn index(skills: &BTreeMap<SkillName, Skill>) -> HashMap<String, SkillName> {
    if skills.len() > SKILL_TOOL_LIMIT {
        return HashMap::new();
    }
    let mut taken = HashSet::with_capacity(skills.len());
    skills
        .keys()
        .map(|name| (tool_name_for(name, &mut taken), name.clone()))
        .collect()
}

/// `skill_<slug>`, within `^[a-zA-Z0-9_-]{1,64}$` for the strictest client, and unique
/// against everything already issued.
pub fn tool_name_for(name: &SkillName, taken: &mut HashSet<String>) -> String {
    let mut slug = String::with_capacity(name.as_str().len());
    let mut last_underscore = false;
    for c in name.as_str().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            slug.push('_');
            last_underscore = true;
        }
    }
    let slug = slug.trim_matches('_');
    let base: String = slug.chars().take(56).collect();
    let base = if base.is_empty() {
        "unnamed".to_string()
    } else {
        base
    };
    let mut candidate = format!("{PREFIX}{base}");
    let mut n = 2;
    while !taken.insert(candidate.clone()) {
        candidate = format!("{PREFIX}{base}_{n}");
        n += 1;
    }
    candidate
}

/// What a skill's MCP dependency looks like right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpAuthState {
    NotRequired,
    Satisfied,
    NeedsLogin,
    NotConfigured,
    /// The registry is not available to this process.
    Unknown,
}

/// Resolves a skill's declared dependencies. Implemented by the MCP registry; the
/// no-op keeps this module testable and lets the stdio child run before the OAuth
/// machinery exists.
pub trait AuthLookup {
    fn state(&self, server: &str) -> McpAuthState;
}

pub struct NoAuthInfo;

impl AuthLookup for NoAuthInfo {
    fn state(&self, _server: &str) -> McpAuthState {
        McpAuthState::Unknown
    }
}

fn no_args() -> Value {
    json!({ "type": "object", "properties": {}, "required": [] })
}

/// The full `tools/list` payload.
pub fn tool_definitions(store: &SkillStore, auth: &dyn AuthLookup) -> Value {
    let mut tools = vec![
        json!({
            "name": "skill_list",
            "description": "List every available opman skill with its description.",
            "inputSchema": no_args(),
        }),
        json!({
            "name": "skill_load",
            "description": "Load one opman skill's full instructions by name.",
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"],
            },
        }),
    ];
    let skills = store.skills();
    if skills.len() > SKILL_TOOL_LIMIT {
        tracing::debug!(
            count = skills.len(),
            limit = SKILL_TOOL_LIMIT,
            "too many skills for per-skill tools; exposing the list/load pair only"
        );
        return Value::Array(tools);
    }
    let mut taken: HashSet<String> = tools
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    for (name, skill) in skills {
        tools.push(json!({
            "name": tool_name_for(name, &mut taken),
            "description": describe(skill, auth),
            "inputSchema": no_args(),
        }));
    }
    Value::Array(tools)
}

/// A skill's description, plus what the model needs to know *before* selecting it.
fn describe(skill: &Skill, auth: &dyn AuthLookup) -> String {
    let mut text = skill.description.clone();
    for server in &skill.requires {
        let note = match auth.state(server) {
            McpAuthState::NeedsLogin => format!(
                " Requires the \"{server}\" MCP server, which is not authenticated — \
                 the user must run `opman mcp login {server}`."
            ),
            McpAuthState::NotConfigured => {
                format!(" Requires the \"{server}\" MCP server, which is not configured in opman.")
            }
            McpAuthState::Satisfied | McpAuthState::NotRequired | McpAuthState::Unknown => {
                format!(" Uses the \"{server}\" MCP server.")
            }
        };
        text.push_str(&note);
    }
    text
}

fn text_result(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

/// Run one `tools/call`.
pub fn dispatch_tool(store: &SkillStore, auth: &dyn AuthLookup, params: Option<&Value>) -> Value {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    match name {
        "skill_list" => {
            let listing: Vec<Value> = store
                .skills()
                .values()
                .map(|s| json!({ "name": s.name, "title": s.title, "description": s.description }))
                .collect();
            text_result(serde_json::to_string_pretty(&listing).unwrap_or_else(|_| "[]".to_string()))
        }
        "skill_load" => {
            let wanted = params
                .and_then(|p| p.pointer("/arguments/name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            match store.get(wanted) {
                Some(skill) => text_result(body_with_warnings(skill, auth)),
                None => text_result(format!("Skill '{wanted}' not found")),
            }
        }
        other => match store.by_tool(other) {
            Some(skill) => text_result(body_with_warnings(skill, auth)),
            None => text_result(format!("Unknown tool: {other}")),
        },
    }
}

/// The skill body, preceded by any dependency warning.
///
/// Warn rather than refuse: refusing makes an agent flail, while a warning lets it
/// proceed and fail informatively on the first call that actually needs the credential.
fn body_with_warnings(skill: &Skill, auth: &dyn AuthLookup) -> String {
    let mut warnings = String::new();
    for server in &skill.requires {
        match auth.state(server) {
            McpAuthState::NeedsLogin => warnings.push_str(&format!(
                "> This skill needs the \"{server}\" MCP server, which is not \
                 authenticated. Tell the user to run `opman mcp login {server}`.\n"
            )),
            McpAuthState::NotConfigured => warnings.push_str(&format!(
                "> This skill needs the \"{server}\" MCP server, which is not \
                 configured in opman.\n"
            )),
            _ => {}
        }
    }
    if warnings.is_empty() {
        return skill.content.clone();
    }
    format!("{warnings}\n{}", skill.content)
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tools_tests;
