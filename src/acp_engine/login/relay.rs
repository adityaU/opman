//! Telling an agent's chatter from its protocol, and carrying a browser redirect to it.
//!
//! Some agents log in the way command-line tools do: print a link, then wait for the browser
//! to be sent to a port on this machine. When the browser is on another machine that last
//! step cannot happen, so the user pastes the address the browser failed to open and opman
//! requests it here, where the port is.

use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

const DELIVERY_LIMIT: Duration = Duration::from_secs(10);
const LOOPBACK_PREFIXES: [&str; 3] = ["http://127.0.0.1", "http://localhost", "http://[::1]"];

/// One line from the agent.
#[derive(Debug, PartialEq)]
pub(super) enum Line {
    /// A JSON-RPC message.
    Frame(Value),
    /// Anything else: a link to open, a progress note, a log line.
    Text(String),
}

pub(super) fn classify(line: &str) -> Line {
    match serde_json::from_str::<Value>(line) {
        Ok(value) if value.get("jsonrpc").is_some() => Line::Frame(value),
        _ => Line::Text(line.to_string()),
    }
}

/// Whether the text points the browser at a port on this machine, which is the sign that a
/// remote browser will need the paste step.
pub(super) fn mentions_loopback(text: &str) -> bool {
    text.contains("127.0.0.1") || text.contains("localhost") || text.contains("%2F%2F127.0.0.1")
}

/// What the user typed, when it is an address on this machine.
pub(super) fn redirect_target(input: &str) -> Option<&str> {
    let input = input.trim();
    // The host must end at the prefix: `http://127.0.0.1.evil.example` is not this machine.
    let local = LOOPBACK_PREFIXES.iter().any(|prefix| {
        input
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/', '?']))
    });
    local.then_some(input)
}

/// Request the redirect. Any HTTP answer counts: the agent's local server replies with a
/// "you can close this tab" page, and what matters is that the request reached it.
pub(super) async fn deliver(url: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(DELIVERY_LIMIT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("cannot build the HTTP client")?;
    client
        .get(url)
        .send()
        .await
        .with_context(|| format!("could not reach {url}; is the login still waiting?"))?;
    Ok(())
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod relay_tests;
