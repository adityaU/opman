//! Tool naming, the per-skill surface, and dependency surfacing.

use std::collections::HashSet;

use serde_json::json;

use super::*;
use crate::mcp_skills::name::SkillName;
use crate::mcp_skills::store::SkillStore;
use crate::mcp_skills::Skill;

fn name(raw: &str) -> SkillName {
    SkillName::parse(raw).expect("valid name")
}

fn skill(n: &str, description: &str, requires: Vec<String>) -> Skill {
    Skill {
        name: name(n),
        title: n.to_string(),
        description: description.to_string(),
        content: "BODY".to_string(),
        requires,
    }
}

fn store_with(skills: Vec<Skill>) -> SkillStore {
    SkillStore::seeded(skills)
}

struct Fixed(McpAuthState);

impl AuthLookup for Fixed {
    fn state(&self, _server: &str) -> McpAuthState {
        self.0.clone()
    }
}

// -- tool naming ---------------------------------------------------------------------

#[test]
fn a_tool_name_is_prefixed_and_slugged() {
    let mut taken = HashSet::new();
    assert_eq!(
        tool_name_for(&name("jira-triage"), &mut taken),
        "skill_jira_triage"
    );
}

#[test]
fn colliding_slugs_are_disambiguated() {
    let mut taken = HashSet::new();
    assert_eq!(tool_name_for(&name("a-b"), &mut taken), "skill_a_b");
    assert_eq!(tool_name_for(&name("a.b"), &mut taken), "skill_a_b_2");
    assert_eq!(tool_name_for(&name("a_b"), &mut taken), "skill_a_b_3");
}

#[test]
fn a_long_name_is_truncated_within_the_strictest_limit() {
    let mut taken = HashSet::new();
    let tool = tool_name_for(&name(&"a".repeat(64)), &mut taken);
    assert!(tool.len() <= 64, "{tool} is {} chars", tool.len());
}

// -- the surface ---------------------------------------------------------------------

#[test]
fn the_pair_is_always_present() {
    let store = store_with(Vec::new());
    let tools = tool_definitions(&store, &NoAuthInfo);
    let names: Vec<_> = tools
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names, ["skill_list", "skill_load"]);
}

#[test]
fn each_skill_gets_its_own_tool() {
    let store = store_with(vec![
        skill("alpha", "A", vec![]),
        skill("beta", "B", vec![]),
    ]);
    let tools = tool_definitions(&store, &NoAuthInfo);
    let names: Vec<_> = tools
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"skill_alpha"));
    assert!(names.contains(&"skill_beta"));
}

/// Past the limit the per-skill tools would crowd out the runner's own, so the pair
/// carries it instead.
#[test]
fn past_the_limit_only_the_pair_is_offered() {
    let skills = (0..=SKILL_TOOL_LIMIT)
        .map(|i| skill(&format!("s{i}"), "d", vec![]))
        .collect();
    let store = store_with(skills);
    let tools = tool_definitions(&store, &NoAuthInfo);
    assert_eq!(tools.as_array().expect("array").len(), 2);
}

// -- dependency surfacing ------------------------------------------------------------

#[test]
fn a_missing_login_is_named_in_the_description() {
    // The model has to learn this before selecting, not after.
    let store = store_with(vec![skill("jira", "Triage.", vec!["jira".into()])]);
    let tools = tool_definitions(&store, &Fixed(McpAuthState::NeedsLogin));
    let described = tools
        .as_array()
        .expect("array")
        .iter()
        .find(|t| t["name"] == "skill_jira")
        .expect("tool")["description"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(described.contains("not authenticated"));
    assert!(described.contains("opman mcp login jira"));
}

/// Warn, do not refuse: refusing makes the agent flail, while a warning lets it proceed
/// and fail informatively on the first call that actually needs the credential.
#[test]
fn calling_a_skill_that_needs_a_login_still_returns_the_body() {
    let store = store_with(vec![skill("jira", "d", vec!["jira".into()])]);
    let result = dispatch_tool(
        &store,
        &Fixed(McpAuthState::NeedsLogin),
        Some(&json!({ "name": "skill_jira" })),
    );
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("BODY"), "the body must still be delivered");
    assert!(text.contains("opman mcp login jira"));
}

#[test]
fn a_satisfied_dependency_adds_no_warning_to_the_body() {
    let store = store_with(vec![skill("jira", "d", vec!["jira".into()])]);
    let result = dispatch_tool(
        &store,
        &Fixed(McpAuthState::Satisfied),
        Some(&json!({ "name": "skill_jira" })),
    );
    assert_eq!(result["content"][0]["text"], "BODY");
}

// -- dispatch ------------------------------------------------------------------------

#[test]
fn skill_list_returns_every_skill() {
    let store = store_with(vec![skill("alpha", "A", vec![])]);
    let result = dispatch_tool(&store, &NoAuthInfo, Some(&json!({ "name": "skill_list" })));
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("alpha"));
}

#[test]
fn skill_load_takes_a_name_argument() {
    let store = store_with(vec![skill("alpha", "A", vec![])]);
    let result = dispatch_tool(
        &store,
        &NoAuthInfo,
        Some(&json!({ "name": "skill_load", "arguments": { "name": "alpha" } })),
    );
    assert_eq!(result["content"][0]["text"], "BODY");
}

#[test]
fn an_unknown_skill_reports_rather_than_erroring() {
    let store = store_with(Vec::new());
    let result = dispatch_tool(
        &store,
        &NoAuthInfo,
        Some(&json!({ "name": "skill_load", "arguments": { "name": "nope" } })),
    );
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("not found"));
}

#[test]
fn an_unknown_tool_reports_rather_than_erroring() {
    let store = store_with(Vec::new());
    let result = dispatch_tool(&store, &NoAuthInfo, Some(&json!({ "name": "nope" })));
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("Unknown tool"));
}

#[test]
fn a_traversal_name_cannot_reach_a_skill() {
    let store = store_with(vec![skill("alpha", "A", vec![])]);
    let result = dispatch_tool(
        &store,
        &NoAuthInfo,
        Some(&json!({ "name": "skill_load", "arguments": { "name": "../alpha" } })),
    );
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("not found"));
}
