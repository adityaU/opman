use tokio::sync::mpsc;

use super::super::config::AgentConfig;
use super::input::Input;
use super::*;

fn agent(script: &str) -> AgentConfig {
    AgentConfig {
        command: "sh".into(),
        args: vec!["-c".into(), script.into()],
        ..AgentConfig::default()
    }
}

fn no_input() -> Input {
    let (_keep, lines) = mpsc::channel(1);
    Input::fed(lines)
}

const INIT: &str = r#"read a; printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"authMethods":[{"id":"oauth","name":"Log in"}]}}'"#;

#[tokio::test]
async fn an_agent_method_is_authenticated_and_its_chatter_is_shown() {
    let script = format!(
        r#"{INIT}
read b
echo 'Open this link: http://127.0.0.1:5/login'
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{}}}}'"#
    );
    let mut shown = Vec::new();
    drive(&agent(&script), "/tmp", None, no_input(), &mut shown)
        .await
        .expect("login");
    let shown = String::from_utf8_lossy(&shown);
    assert!(shown.contains("Open this link"), "{shown}");
    assert!(shown.contains("paste it here"), "the paste hint is shown once: {shown}");
}

#[tokio::test]
async fn a_refused_login_reports_the_agents_reason() {
    let script = format!(
        r#"{INIT}
read b
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"error":{{"code":-32000,"message":"Onboarding failed"}}}}'"#
    );
    let mut shown = Vec::new();
    let error = drive(&agent(&script), "/tmp", None, no_input(), &mut shown)
        .await
        .expect_err("refused");
    assert!(format!("{error:#}").contains("Onboarding failed"));
}

#[tokio::test]
async fn several_methods_ask_and_the_answer_picks_one() {
    let script = r#"read a; printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"authMethods":[{"id":"one","name":"One"},{"id":"two","name":"Two"}]}}'
read b
case "$b" in *'"two"'*) printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{}}';; *) printf '%s\n' '{"jsonrpc":"2.0","id":2,"error":{"message":"wrong method"}}';; esac"#;
    let (tx, lines) = mpsc::channel(4);
    tx.send("2\n".to_string()).await.expect("send");
    let mut shown = Vec::new();
    drive(&agent(script), "/tmp", None, Input::fed(lines), &mut shown)
        .await
        .expect("login with the second method");
    assert!(String::from_utf8_lossy(&shown).contains("2. Two"));
}

#[tokio::test]
async fn an_unknown_method_name_fails_before_logging_in() {
    let script = format!("{INIT}\nsleep 5");
    let mut shown = Vec::new();
    let error = drive(&agent(&script), "/tmp", Some("nope"), no_input(), &mut shown)
        .await
        .expect_err("unknown method");
    assert!(format!("{error:#}").contains("no login method `nope`"));
}

#[test]
fn replies_are_matched_by_id_and_requests_are_ignored() {
    let request = json!({ "jsonrpc": "2.0", "id": 2, "method": "fs/read", "params": {} });
    assert!(answer(&request, 2, "x").is_none());
    let other = json!({ "jsonrpc": "2.0", "id": 9, "result": {} });
    assert!(answer(&other, 2, "x").is_none());
}
