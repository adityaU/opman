//! Skills REST handlers: the read paths, auth, and name validation.
//!
//! The FS-writing paths live in `skills_handlers_fs_tests.rs`, which redirects
//! `XDG_CONFIG_HOME` under a lock.

use super::*;

use axum::extract::State;

use crate::mcp_skills::{Skill, SkillName, SkillsRegistry};
use crate::web::test_support::test_server_state;

fn name(raw: &str) -> SkillName {
    SkillName::parse(raw).expect("valid name")
}

fn skill(n: &str, description: &str) -> Skill {
    Skill {
        name: name(n),
        title: n.to_string(),
        description: description.to_string(),
        content: "BODY".to_string(),
        requires: Vec::new(),
    }
}

async fn state_with(skills: Vec<Skill>) -> crate::web::types::ServerState {
    let state = test_server_state();
    let registry: &SkillsRegistry = &state.skills_registry;
    let mut guard = registry.write().await;
    for s in skills {
        guard.insert(s.name.clone(), s);
    }
    drop(guard);
    state
}

/// The extractor is a no-op when auth is not configured, which is what `test_server_state`
/// produces — so these exercise the handler bodies, not the auth gate.
fn open() -> AuthUser {
    AuthUser {
        subject: String::new(),
    }
}

#[tokio::test]
async fn list_returns_every_skill_in_name_order() {
    let state = state_with(vec![skill("zeta", "Z"), skill("alpha", "A")]).await;
    let axum::Json(list) = list_skills(open(), State(state)).await.expect("ok");
    let names: Vec<_> = list.iter().map(|s| s.name.as_str().to_string()).collect();
    // BTreeMap-backed, so the UI list stops reshuffling between fetches.
    assert_eq!(names, ["alpha", "zeta"]);
}

#[tokio::test]
async fn list_carries_the_fields_the_ui_needs() {
    let mut demo = skill("demo", "D");
    demo.title = "Demo Skill".into();
    demo.requires = vec!["jira".into()];
    let state = state_with(vec![demo]).await;
    let axum::Json(list) = list_skills(open(), State(state)).await.expect("ok");
    assert_eq!(list[0].title, "Demo Skill");
    assert_eq!(list[0].requires, ["jira"]);
}

#[tokio::test]
async fn get_returns_the_skill() {
    let state = state_with(vec![skill("demo", "D")]).await;
    let axum::Json(found) = get_skill(open(), State(state), Path(name("demo")))
        .await
        .expect("ok");
    assert_eq!(found.expect("present").content, "BODY");
}

#[tokio::test]
async fn get_returns_null_for_an_unknown_skill() {
    let state = state_with(Vec::new()).await;
    let axum::Json(found) = get_skill(open(), State(state), Path(name("nope")))
        .await
        .expect("ok");
    assert!(found.is_none());
}

/// The traversal guard is the deserializer, so an invalid name never reaches a handler.
#[test]
fn a_traversal_name_cannot_be_deserialized_into_a_request() {
    let body = r#"{"name":"../evil","description":"d","content":"c"}"#;
    assert!(serde_json::from_str::<CreateSkillRequest>(body).is_err());

    let ok = r#"{"name":"fine","description":"d","content":"c"}"#;
    assert!(serde_json::from_str::<CreateSkillRequest>(ok).is_ok());
}

#[test]
fn a_request_renders_frontmatter_that_survives_a_round_trip() {
    let body = r#"{"name":"demo","description":"Fix: it","content":"BODY","requires":["jira"]}"#;
    let req: CreateSkillRequest = serde_json::from_str(body).expect("parses");
    let rendered = req.render().expect("renders");
    let parsed = crate::mcp_skills::format::parse_skill_md(&rendered, &name("demo"))
        .expect("round trips");
    assert_eq!(parsed.description, "Fix: it");
    assert_eq!(parsed.requires, ["jira"]);
}

#[tokio::test]
async fn create_rejects_an_empty_description() {
    let state = state_with(Vec::new()).await;
    let req: CreateSkillRequest =
        serde_json::from_str(r#"{"name":"demo","description":"","content":"c"}"#).expect("parses");
    let result = create_skill(open(), State(state), axum::Json(req)).await;
    assert!(matches!(result, Err(StatusCode::BAD_REQUEST)));
}
