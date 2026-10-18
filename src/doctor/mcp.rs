//! Every MCP server in the registry: launch it through the real probe and count its tools.

use std::time::Duration;

use crate::mcp_probe::{self, Catalog};
use crate::mcp_registry::{BuiltinFlags, McpRegistry};

use super::report::{Check, Section};

/// A little more than the probe's own limit, so the probe reports its own timeout.
const OUTER_LIMIT: Duration = Duration::from_secs(25);

pub(super) async fn section() -> Section {
    let registry = McpRegistry::load(BuiltinFlags::default());
    let dir = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string());
    let names: Vec<&str> = registry.all().map(|spec| spec.name()).collect();
    if names.is_empty() {
        return Section::new(
            "MCP servers",
            vec![Check::pass("no MCP servers are declared")],
        );
    }
    let probes = names.iter().map(|name| probe(&registry, name, &dir));
    Section::new("MCP servers", futures::future::join_all(probes).await)
}

async fn probe(registry: &McpRegistry, name: &str, dir: &str) -> Check {
    let work = mcp_probe::catalog(registry, name, dir);
    match tokio::time::timeout(OUTER_LIMIT, work).await {
        Ok(catalog) => describe(name, catalog),
        Err(_) => Check::fail(format!(
            "{name}: no answer within {} seconds",
            OUTER_LIMIT.as_secs()
        )),
    }
}

pub(super) fn describe(name: &str, catalog: Catalog) -> Check {
    match catalog {
        Catalog::Listed { server, tools } => {
            let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
            let who = server
                .map(|info| format!("{} {}", info.name, info.version.unwrap_or_default()))
                .unwrap_or_default();
            let title = format!("{name}: {} tools", tools.len());
            if tools.is_empty() {
                return Check::warn(format!("{title} (the server listed none)")).detail(who.trim());
            }
            Check::pass(title).detail(format!("{}\n{}", who.trim(), names.join(", ")))
        }
        Catalog::Unavailable { reason } => {
            Check::warn(format!("{name}: not launchable right now")).detail(reason)
        }
        Catalog::Failed { reason } => {
            Check::fail(format!("{name}: failed to answer")).detail(reason)
        }
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod mcp_tests;
