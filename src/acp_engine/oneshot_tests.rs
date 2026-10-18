use std::time::Duration;

use super::super::config::AgentConfig;
use super::{ask, Failure};

/// A shell script that answers the three requests `ask` makes, in order.
fn agent(script: &str) -> AgentConfig {
    AgentConfig {
        command: "sh".into(),
        args: vec!["-c".into(), script.into()],
        ..AgentConfig::default()
    }
}

const LIMIT: Duration = Duration::from_secs(10);

const HAPPY: &str = r#"
read a; echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}'
read b; echo '{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1","plan":"pro"}}'
read c
echo '{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hel"}}}}'
echo '{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"skip"}}}}'
echo '{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"lo"}}}}'
echo '{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}'
"#;

#[tokio::test]
async fn collects_only_message_text_and_keeps_the_session_result() {
    let answer = ask(&agent(HAPPY), "/usage", LIMIT)
        .await
        .expect("test fixture or assertion");
    assert_eq!(answer.text, "hello");
    assert_eq!(answer.session["plan"], "pro");
}

#[tokio::test]
async fn auth_required_is_its_own_failure() {
    let script = r#"
read a; echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read b; echo '{"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"Authentication required"}}'
"#;
    let failure = ask(&agent(script), "/usage", LIMIT).await;
    assert_eq!(failure, Err(Failure::LoginRequired));
}

#[tokio::test]
async fn an_agent_that_exits_early_gives_its_stderr() {
    let failure = ask(&agent("echo boom >&2; exit 3"), "/usage", LIMIT).await;
    let Err(Failure::Other(reason)) = failure else {
        panic!("expected a failure with a reason");
    };
    assert!(reason.contains("boom"), "{reason}");
}

#[tokio::test]
async fn a_silent_agent_times_out() {
    let failure = ask(&agent("sleep 30"), "/usage", Duration::from_millis(300)).await;
    let Err(Failure::Other(reason)) = failure else {
        panic!("expected a timeout");
    };
    assert!(reason.contains("did not answer"), "{reason}");
}
