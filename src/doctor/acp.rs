//! ACP agents: can each enabled one start, answer `initialize`, and open a session?
//!
//! The probing itself is `acp_engine::diagnose`, which runs the engine's own start-up path.
//! This file turns its answers into checks.

use std::path::Path;
use std::time::Duration;

use crate::acp_engine::catalog;
use crate::acp_engine::config::{AcpConfig, AgentConfig};
use crate::acp_engine::diagnose::{self, Hello, Offer, Session};

use super::report::{Check, Section};

/// How long an agent gets for each of `initialize` and `session/new`.
pub(super) const STEP_LIMIT: Duration = Duration::from_secs(20);
/// Long lists are cut so one agent with 80 models does not bury the report.
const SHOWN_NAMES: usize = 12;

/// The agents to check: the one asked for, or every enabled one.
pub(super) fn selected<'a>(
    config: &'a AcpConfig,
    only: Option<&str>,
) -> Result<Vec<(&'a str, &'a AgentConfig)>, Check> {
    let Some(wanted) = only else {
        return Ok(config
            .active()
            .map(|(id, agent)| (id.as_str(), agent))
            .collect());
    };
    match config.agents.get_key_value(wanted) {
        Some((id, agent)) => Ok(vec![(id.as_str(), agent)]),
        None => {
            let known: Vec<&str> = config.agents.keys().map(String::as_str).collect();
            Err(Check::fail(format!("no ACP agent named `{wanted}`"))
                .detail(format!("Known agents: {}", known.join(", "))))
        }
    }
}

pub(super) async fn section(agents: Result<Vec<(&str, &AgentConfig)>, Check>) -> Section {
    let agents = match agents {
        Ok(agents) => agents,
        Err(check) => return Section::new("ACP agents", vec![check]),
    };
    if agents.is_empty() {
        let check = Check::warn("no ACP agent is enabled")
            .detail("Enable one in Settings, or in ~/.config/opman/acp.json.");
        return Section::new("ACP agents", vec![check]);
    }
    let runs = agents
        .into_iter()
        .map(|(id, agent)| agent_checks(id, agent, STEP_LIMIT));
    let checks = futures::future::join_all(runs)
        .await
        .into_iter()
        .flatten()
        .collect();
    Section::new("ACP agents", checks)
}

/// All checks for one agent. The probe starts a real process and always stops it again.
pub(super) async fn agent_checks(id: &str, agent: &AgentConfig, limit: Duration) -> Vec<Check> {
    if agent.command.is_empty() {
        return vec![Check::warn(format!("{id}: no launch command is set"))
            .detail("The catalogue has no documented command for this agent. Set `command` in acp.json.")];
    }
    if let Some(check) = not_installed(id, agent) {
        return vec![check];
    }
    let line = command_line(agent);
    match diagnose::probe(id, agent, limit).await {
        Err(reason) => vec![Check::fail(format!("{id}: could not start"))
            .detail(format!("Command: {line}\n{reason}"))],
        Ok(probe) => vec![
            hello_check(id, &line, &probe.hello),
            session_check(id, agent, &probe.hello, &probe.session),
        ],
    }
}

fn command_line(agent: &AgentConfig) -> String {
    std::iter::once(agent.command.as_str())
        .chain(agent.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// An archive agent that has not been downloaded yet. Doctor never downloads it.
fn not_installed(id: &str, agent: &AgentConfig) -> Option<Check> {
    let archive = catalog::archive_for(&agent.runner)?;
    let (command, _) = archive.launch(&agent.runner)?;
    if Path::new(&agent.command) != Path::new(&command) || Path::new(&command).is_file() {
        return None;
    }
    Some(
        Check::warn(format!("{id}: not installed yet")).detail(format!(
            "opman downloads version {} on the first start, to {command}.",
            archive.version
        )),
    )
}

fn hello_check(id: &str, line: &str, hello: &Hello) -> Check {
    let who = match (&hello.name, &hello.version) {
        (Some(name), Some(version)) => format!("{name} {version}"),
        (Some(name), None) => name.clone(),
        _ => "unnamed agent".to_string(),
    };
    let auth = if hello.auth_methods.is_empty() {
        "none".to_string()
    } else {
        hello.auth_methods.join(", ")
    };
    let resume = if hello.loads_sessions { "yes" } else { "no" };
    Check::pass(format!(
        "{id}: initialize ok, {who}, ACP v{}",
        hello.protocol
    ))
    .detail(format!(
        "Command: {line}\nAuth methods: {auth}\nCan reopen old sessions: {resume}"
    ))
}

fn session_check(id: &str, agent: &AgentConfig, hello: &Hello, session: &Session) -> Check {
    match session {
        Session::Offered(offer) => offered(id, agent, offer),
        Session::LoginRequired => {
            let methods = hello.auth_methods.join(", ");
            Check::warn(format!("{id}: login required before a session can start"))
                .detail(format!("The agent answered `session/new` with auth_required. Log in with the agent's own CLI. Auth methods: {methods}"))
        }
        Session::Failed(reason) => Check::fail(format!("{id}: session/new failed")).detail(reason),
    }
}

fn offered(id: &str, agent: &AgentConfig, offer: &Offer) -> Check {
    let kind = if agent.modes_are_agents {
        "agents"
    } else {
        "permission modes"
    };
    let title = format!(
        "{id}: session ok, {} models, {} {kind}, {} effort levels",
        offer.models.len(),
        offer.modes.len(),
        offer.efforts.len()
    );
    let current = offer
        .current_model
        .as_deref()
        .map(|m| format!(" (current: {m})"))
        .unwrap_or_default();
    let detail = format!(
        "Models{current}: {}\n{}: {}\nEffort levels: {}",
        list(&offer.models),
        capitalised(kind),
        list(&offer.modes),
        list(&offer.efforts)
    );
    let missing = match (offer.models.is_empty(), offer.modes.is_empty()) {
        (true, true) => "no models and no modes",
        (true, false) => "no models",
        (false, true) => "no modes",
        (false, false) => return Check::pass(title).detail(detail),
    };
    Check::warn(format!("{title}, but the agent offers {missing}")).detail(detail)
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn list(names: &[String]) -> String {
    if names.is_empty() {
        return "none".to_string();
    }
    let shown = names
        .iter()
        .take(SHOWN_NAMES)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().checked_sub(SHOWN_NAMES) {
        Some(more) if more > 0 => format!("{shown}, and {more} more"),
        _ => shown,
    }
}

#[cfg(test)]
#[path = "acp_tests.rs"]
mod acp_tests;
