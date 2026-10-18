use super::*;
use crate::doctor::report::Status;
use std::os::unix::fs::PermissionsExt;

fn sh_agent(dir: &Path, body: &str) -> AgentConfig {
    let file = dir.join("agent.sh");
    std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    AgentConfig {
        command: file.to_string_lossy().into_owned(),
        ..AgentConfig::default()
    }
}

/// A shell script that answers `initialize` and `session/new` like a small ACP agent.
const GOOD_AGENT: &str = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) echo '{"jsonrpc":"2.0","id":'$id',"result":{"protocolVersion":1,"agentInfo":{"name":"fake","version":"9"},"authMethods":[{"id":"login","name":"Log in"}]}}';;
    *'"session/new"'*) echo '{"jsonrpc":"2.0","id":'$id',"result":{"sessionId":"s1","modes":{"currentModeId":"default","availableModes":[{"id":"default","name":"Default"},{"id":"plan","name":"Plan"}]},"models":{"currentModelId":"m1","availableModels":[{"modelId":"m1","name":"Model One"}]}}}';;
  esac
done
"#;

#[tokio::test]
async fn a_good_agent_reports_models_and_modes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let agent = sh_agent(dir.path(), GOOD_AGENT);
    let checks = agent_checks("fake", &agent, Duration::from_secs(10)).await;
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].status, Status::Pass);
    assert!(
        checks[0].title.contains("fake 9, ACP v1"),
        "{}",
        checks[0].title
    );
    assert!(checks[0]
        .detail
        .as_deref()
        .unwrap_or("")
        .contains("Auth methods: Log in"));
    assert_eq!(checks[1].status, Status::Pass);
    assert!(
        checks[1].title.contains("1 models, 2 permission modes"),
        "{}",
        checks[1].title
    );
}

#[tokio::test]
async fn an_agent_that_exits_at_once_shows_its_stderr_and_exit_status() {
    let dir = tempfile::tempdir().expect("temp dir");
    let agent = sh_agent(dir.path(), "echo 'Error: missing token' >&2; exit 3");
    let checks = agent_checks("fake", &agent, Duration::from_secs(10)).await;
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].status, Status::Fail);
    let detail = checks[0].detail.as_deref().unwrap_or("");
    assert!(detail.contains("Error: missing token"), "{detail}");
    assert!(detail.contains("exit status: 3"), "{detail}");
}

#[tokio::test]
async fn a_silent_agent_times_out_with_the_step_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let agent = sh_agent(dir.path(), "exec sleep 30");
    let checks = agent_checks("fake", &agent, Duration::from_millis(300)).await;
    assert_eq!(checks[0].status, Status::Fail);
    assert!(checks[0]
        .detail
        .as_deref()
        .unwrap_or("")
        .contains("did not answer `initialize`"));
}

#[tokio::test]
async fn a_missing_binary_says_it_is_not_on_the_path() {
    let agent = AgentConfig {
        command: "opman-no-such-agent-binary".into(),
        ..AgentConfig::default()
    };
    let checks = agent_checks("ghost", &agent, Duration::from_secs(5)).await;
    assert_eq!(checks[0].status, Status::Fail);
    assert!(checks[0]
        .detail
        .as_deref()
        .unwrap_or("")
        .contains("not found on PATH"));
}

#[tokio::test]
async fn an_agent_without_a_command_is_a_warning() {
    let checks = agent_checks("blank", &AgentConfig::default(), Duration::from_secs(1)).await;
    assert_eq!(checks[0].status, Status::Warn);
}

#[test]
fn login_required_is_a_warning_that_names_the_methods() {
    let hello = Hello {
        protocol: 1,
        name: None,
        version: None,
        auth_methods: vec!["oauth".into()],
        loads_sessions: false,
    };
    let check = session_check(
        "a",
        &AgentConfig::default(),
        &hello,
        &Session::LoginRequired,
    );
    assert_eq!(check.status, Status::Warn);
    assert!(check.detail.as_deref().unwrap_or("").contains("oauth"));
}

#[test]
fn an_agent_with_no_models_gets_a_warning() {
    let offer = Offer {
        modes: vec!["default".into()],
        ..Offer::default()
    };
    let check = offered("a", &AgentConfig::default(), &offer);
    assert_eq!(check.status, Status::Warn);
    assert!(check.title.ends_with("but the agent offers no models"));
}

#[test]
fn modes_are_called_agents_when_the_config_says_so() {
    let agent = AgentConfig {
        modes_are_agents: true,
        ..AgentConfig::default()
    };
    let offer = Offer {
        models: vec!["m".into()],
        modes: vec!["build".into()],
        ..Offer::default()
    };
    assert!(offered("a", &agent, &offer).title.contains("1 agents"));
}

#[test]
fn long_lists_are_cut() {
    let names: Vec<String> = (0..15).map(|n| n.to_string()).collect();
    assert!(list(&names).ends_with("and 3 more"));
    assert_eq!(list(&[]), "none");
}

#[test]
fn an_unknown_agent_id_is_a_failure_that_lists_the_known_ones() {
    let config = crate::acp_engine::config::load();
    let check = selected(&config, Some("nope")).expect_err("unknown id");
    assert_eq!(check.status, Status::Fail);
    assert!(check.detail.as_deref().unwrap_or("").contains("claude"));
    assert_eq!(selected(&config, Some("claude")).expect("known").len(), 1);
}
