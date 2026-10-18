//! The probe's two requests, against an in-process agent that answers like a real one.

use super::*;

use crate::acp_engine::config::AgentConfig;

const LIMIT: Duration = Duration::from_secs(5);

fn test_engine() -> Arc<AcpEngine> {
    engine("test", &AgentConfig::default())
}

fn good_agent(method: &str, _params: &Value) -> std::result::Result<Value, Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": 1,
            "agentInfo": { "name": "fake", "version": "2.0" },
            "authMethods": [{ "id": "key", "name": "API key" }],
            "agentCapabilities": { "loadSession": true },
        })),
        "session/new" => Ok(json!({
            "sessionId": "s1",
            "modes": { "currentModeId": "default", "availableModes": [
                { "id": "default", "name": "Default" }, { "id": "plan", "name": "Plan" } ] },
            "models": { "currentModelId": "m1", "availableModels": [
                { "modelId": "m1", "name": "One" }, { "modelId": "m2", "name": "Two" } ] },
        })),
        _ => Err(json!({ "code": -32601, "message": "no" })),
    }
}

#[tokio::test]
async fn a_good_agent_is_described_in_full() {
    let peer = jsonrpc::fake_agent(good_agent);
    let probe = exchange(&test_engine(), &peer, "/tmp", LIMIT)
        .await
        .expect("probe");
    assert_eq!(probe.hello.name.as_deref(), Some("fake"));
    assert_eq!(probe.hello.version.as_deref(), Some("2.0"));
    assert_eq!(probe.hello.auth_methods, vec!["API key"]);
    assert!(probe.hello.loads_sessions);
    let Session::Offered(offer) = probe.session else {
        panic!("expected an offer")
    };
    assert_eq!(offer.models, vec!["One", "Two"]);
    assert_eq!(offer.modes, vec!["Default", "Plan"]);
}

#[tokio::test]
async fn auth_required_is_reported_not_retried() {
    let peer = jsonrpc::fake_agent(|method, params| match method {
        "session/new" => Err(json!({ "code": jsonrpc::AUTH_REQUIRED, "message": "log in" })),
        other => good_agent(other, params),
    });
    let probe = exchange(&test_engine(), &peer, "/tmp", LIMIT)
        .await
        .expect("probe");
    assert_eq!(probe.session, Session::LoginRequired);
}

#[tokio::test]
async fn a_refused_session_keeps_the_message_and_data() {
    let peer = jsonrpc::fake_agent(|method, params| match method {
        "session/new" => Err(
            json!({ "code": -32603, "message": "Internal error", "data": { "details": "no home dir" } }),
        ),
        other => good_agent(other, params),
    });
    let probe = exchange(&test_engine(), &peer, "/tmp", LIMIT)
        .await
        .expect("probe");
    assert_eq!(
        probe.session,
        Session::Failed("Internal error: no home dir".to_string())
    );
}

#[tokio::test]
async fn a_failed_initialize_is_an_error_with_the_agents_reason() {
    let peer = jsonrpc::fake_agent(|_, _| Err(json!({ "code": -32603, "message": "boom" })));
    let error = exchange(&test_engine(), &peer, "/tmp", LIMIT)
        .await
        .expect_err("fails");
    assert!(format!("{error:#}").contains("boom"));
}

#[tokio::test]
async fn an_agent_from_the_future_is_rejected_at_initialize() {
    let peer = jsonrpc::fake_agent(|_, _| Ok(json!({ "protocolVersion": 9 })));
    let error = exchange(&test_engine(), &peer, "/tmp", LIMIT)
        .await
        .expect_err("too new");
    assert!(format!("{error:#}").contains("speaks ACP v9"));
}
