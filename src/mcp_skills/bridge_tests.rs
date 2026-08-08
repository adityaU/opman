//! The stdio loop, driven over in-memory buffers.

use super::*;
use crate::mcp_skills::name::SkillName;
use crate::mcp_skills::Skill;

fn skill(n: &str) -> Skill {
    Skill {
        name: SkillName::parse(n).expect("valid"),
        title: n.to_string(),
        description: format!("does {n}"),
        content: "BODY".to_string(),
        requires: Vec::new(),
    }
}

/// Drive the loop with a script of lines and collect the responses.
async fn drive(skills: Vec<Skill>, lines: &[&str]) -> Vec<Value> {
    let input = lines.join("\n") + "\n";
    let mut output: Vec<u8> = Vec::new();
    run_skills_over(
        SkillStore::seeded(skills),
        Box::new(NoAuthInfo),
        std::io::Cursor::new(input.into_bytes()),
        &mut output,
    )
    .await
    .expect("loop runs to EOF");
    String::from_utf8_lossy(&output)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each response is json"))
        .collect()
}

#[tokio::test]
async fn initialize_reports_the_skills_server() {
    let out = drive(
        Vec::new(),
        &[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#],
    )
    .await;
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["result"]["serverInfo"]["name"], "opman-skills");
    assert_eq!(out[0]["result"]["protocolVersion"], PROTOCOL);
    assert_eq!(out[0]["id"], 1);
}

/// The regression this server exists to avoid: the HTTP handler it replaces answered
/// `notifications/initialized` with `-32601`, and a real MCP client sends that
/// immediately after `initialize` — many abort the handshake on the error.
#[tokio::test]
async fn an_initialized_notification_produces_no_response_at_all() {
    let out = drive(
        Vec::new(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ],
    )
    .await;
    // Exactly two: the notification contributed nothing.
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["id"], 1);
    assert_eq!(out[1]["id"], 2);
}

#[tokio::test]
async fn tools_list_carries_the_pair_and_one_tool_per_skill() {
    let out = drive(
        vec![skill("alpha")],
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#],
    )
    .await;
    let names: Vec<_> = out[0]["result"]["tools"]
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"skill_list"));
    assert!(names.contains(&"skill_load"));
    assert!(names.contains(&"skill_alpha"));
}

#[tokio::test]
async fn a_skill_tool_call_returns_the_body() {
    let out = drive(
        vec![skill("alpha")],
        &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"skill_alpha"}}"#],
    )
    .await;
    assert_eq!(out[0]["result"]["content"][0]["text"], "BODY");
}

#[tokio::test]
async fn prompts_expose_the_same_skills() {
    let out = drive(
        vec![skill("alpha")],
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"prompts/list"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"alpha"}}"#,
        ],
    )
    .await;
    assert_eq!(out[0]["result"]["prompts"][0]["name"], "alpha");
    assert_eq!(out[1]["result"]["messages"][0]["content"]["text"], "BODY");
}

#[tokio::test]
async fn an_unknown_prompt_is_an_invalid_params_error() {
    let out = drive(
        Vec::new(),
        &[r#"{"jsonrpc":"2.0","id":1,"method":"prompts/get","params":{"name":"nope"}}"#],
    )
    .await;
    assert_eq!(out[0]["error"]["code"], -32602);
}

#[tokio::test]
async fn an_unknown_method_is_method_not_found() {
    let out = drive(Vec::new(), &[r#"{"jsonrpc":"2.0","id":9,"method":"nope"}"#]).await;
    assert_eq!(out[0]["error"]["code"], -32601);
    assert_eq!(out[0]["id"], 9);
}

/// A malformed line must be answered, not fatal: killing the loop would take every
/// skill away from the runner for the rest of the session.
#[tokio::test]
async fn malformed_input_is_answered_and_the_loop_survives() {
    let out = drive(
        Vec::new(),
        &[
            "{ not json",
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#,
        ],
    )
    .await;
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["error"]["code"], -32700);
    assert_eq!(out[1]["id"], 2);
}

#[tokio::test]
async fn blank_lines_are_skipped() {
    let out = drive(
        Vec::new(),
        &[
            "",
            "   ",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        ],
    )
    .await;
    assert_eq!(out.len(), 1);
}
