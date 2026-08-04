use super::*;
use crate::web::types::ServerState;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn registry_with(skills: Vec<Skill>) -> SkillsRegistry {
    let mut map = HashMap::new();
    for s in skills {
        map.insert(s.name.clone(), s);
    }
    Arc::new(RwLock::new(map))
}

fn sample_skill(name: &str) -> Skill {
    Skill {
        name: name.to_string(),
        description: format!("desc of {name}"),
        content: format!("full content of {name}"),
    }
}

// ── get_skills_dir ───────────────────────────────────────────────────────────

#[test]
fn skills_dir_path_shape() {
    let dir = get_skills_dir();
    assert!(dir.ends_with("skills"));
    assert!(dir.to_string_lossy().contains("opman"));
}

// ── parse_skill ──────────────────────────────────────────────────────────────

fn write_skill(dir: &tempfile::TempDir, contents: &str) -> std::path::PathBuf {
    let p = dir.path().join("SKILL.md");
    std::fs::write(&p, contents).unwrap();
    p
}

#[test]
fn parse_skill_valid() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = write_skill(
        &dir,
        "---\nname: my-skill\ndescription: does things\n---\nBody content here\n",
    );
    let skill = parse_skill(&p).unwrap();
    assert_eq!(skill.name, "my-skill");
    assert_eq!(skill.description, "does things");
    assert_eq!(skill.content, "Body content here");
}

#[test]
fn parse_skill_body_with_extra_separators() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = write_skill(
        &dir,
        "---\nname: s\ndescription: d\n---\nline1\n---\nline2\n",
    );
    let skill = parse_skill(&p).unwrap();
    assert_eq!(skill.name, "s");
    assert!(skill.content.contains("line1"));
    assert!(skill.content.contains("---"));
    assert!(skill.content.contains("line2"));
}

#[test]
fn parse_skill_missing_frontmatter() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = write_skill(&dir, "just some text with no frontmatter");
    assert!(parse_skill(&p).is_err());
}

#[test]
fn parse_skill_invalid_yaml() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = write_skill(&dir, "---\nfoo: [1, 2\n---\nbody\n");
    assert!(parse_skill(&p).is_err());
}

#[test]
fn parse_skill_defaults_when_fields_absent() {
    let dir = tempfile::TempDir::new().unwrap();
    let p = write_skill(&dir, "---\nother: 1\n---\nbody\n");
    let skill = parse_skill(&p).unwrap();
    assert_eq!(skill.name, "");
    assert_eq!(skill.description, "");
    assert_eq!(skill.content, "body");
}

#[test]
fn parse_skill_missing_file_errors() {
    let p = std::path::PathBuf::from("/tmp/opman-nonexistent-skill-file.md");
    assert!(parse_skill(&p).is_err());
}

// ── dispatch_tool ────────────────────────────────────────────────────────────

#[tokio::test]
async fn dispatch_list_skills() {
    let reg = registry_with(vec![sample_skill("alpha")]);
    let out = dispatch_tool(&reg, "list_skills", &Value::Null).await;
    let text = out[0]["text"].as_str().unwrap();
    assert!(text.contains("alpha"));
    assert!(text.contains("desc of alpha"));
}

#[tokio::test]
async fn dispatch_load_skill_found() {
    let reg = registry_with(vec![sample_skill("beta")]);
    let out = dispatch_tool(&reg, "load_skill", &serde_json::json!({"name":"beta"})).await;
    assert_eq!(out[0]["text"], "full content of beta");
}

#[tokio::test]
async fn dispatch_load_skill_not_found() {
    let reg = registry_with(vec![]);
    let out = dispatch_tool(&reg, "load_skill", &serde_json::json!({"name":"ghost"})).await;
    assert!(out[0]["text"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn dispatch_load_skill_missing_name() {
    let reg = registry_with(vec![]);
    let out = dispatch_tool(&reg, "load_skill", &Value::Null).await;
    assert!(out[0]["text"].as_str().unwrap().contains("Missing 'name'"));
}

#[tokio::test]
async fn dispatch_unknown_tool() {
    let reg = registry_with(vec![]);
    let out = dispatch_tool(&reg, "frobnicate", &Value::Null).await;
    assert!(out[0]["text"]
        .as_str()
        .unwrap()
        .contains("Unknown tool: frobnicate"));
}

// ── mcp_handler ──────────────────────────────────────────────────────────────

async fn call_handler(state: ServerState, body: Value) -> Value {
    use axum::extract::State;
    use axum::Json;
    let req: McpRequest = serde_json::from_value(body).unwrap();
    let resp = mcp_handler(State(state), Json(req)).await.unwrap();
    serde_json::to_value(&resp.0).unwrap()
}

#[tokio::test]
async fn handler_initialize() {
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(
        state,
        serde_json::json!({"jsonrpc":"2.0","method":"initialize","id":1}),
    )
    .await;
    assert_eq!(v["result"]["serverInfo"]["name"], "opman-skills");
    assert_eq!(v["id"], 1);
    // result present, error omitted (skip_serializing_if).
    assert!(v.get("error").is_none());
}

#[tokio::test]
async fn handler_tools_list() {
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(
        state,
        serde_json::json!({"jsonrpc":"2.0","method":"tools/list","id":2}),
    )
    .await;
    let tools = v["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"list_skills"));
    assert!(names.contains(&"load_skill"));
}

#[tokio::test]
async fn handler_tools_call_list_skills() {
    let state = crate::web::test_support::test_server_state();
    state
        .skills_registry
        .write()
        .await
        .insert("gamma".to_string(), sample_skill("gamma"));
    let v = call_handler(
        state,
        serde_json::json!({
            "jsonrpc":"2.0","method":"tools/call","id":3,
            "params":{"name":"list_skills","arguments":{}}
        }),
    )
    .await;
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("gamma"));
}

#[tokio::test]
async fn handler_tools_call_load_skill() {
    let state = crate::web::test_support::test_server_state();
    state
        .skills_registry
        .write()
        .await
        .insert("delta".to_string(), sample_skill("delta"));
    let v = call_handler(
        state,
        serde_json::json!({
            "jsonrpc":"2.0","method":"tools/call","id":4,
            "params":{"name":"load_skill","arguments":{"name":"delta"}}
        }),
    )
    .await;
    assert_eq!(v["result"]["content"][0]["text"], "full content of delta");
}

#[tokio::test]
async fn handler_unknown_method() {
    let state = crate::web::test_support::test_server_state();
    let v = call_handler(
        state,
        serde_json::json!({"jsonrpc":"2.0","method":"bogus","id":5}),
    )
    .await;
    assert_eq!(v["error"]["code"], -32601);
    assert_eq!(v["error"]["message"], "Method not found");
    // result omitted.
    assert!(v.get("result").is_none());
}

// ── struct serde ─────────────────────────────────────────────────────────────

#[test]
fn skill_roundtrip() {
    let s = sample_skill("x");
    let j = serde_json::to_string(&s).unwrap();
    let back: Skill = serde_json::from_str(&j).unwrap();
    assert_eq!(back.name, "x");
    assert_eq!(back.content, "full content of x");
}
