//! Which registry servers the sandbox may reach.

use crate::mcp_registry::spec::ServerSpec;
use crate::mcp_registry::McpRegistry;

use super::launch::Launch;
use super::pool::{Entry, Opener, Pool};
use crate::mcp_code::names::Namespace;

use std::collections::HashSet;

/// Servers that cannot appear in the sandbox, as a closed set.
///
/// `Code` would recurse. Every other MCP server remains reachable so code mode can act as
/// the single gateway and use the management server to enable or configure MCP servers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Excluded {
    Code,
}

impl Excluded {
    pub const ALL: [Self; 1] = [Self::Code];

    pub const fn server(self) -> &'static str {
        "code"
    }

    pub fn names(name: &str) -> bool {
        Self::ALL.iter().any(|excluded| excluded.server() == name)
    }
}

/// Whether `spec` may be mounted: not excluded, and not hidden by its `codeMode` setting.
pub fn eligible(spec: &ServerSpec) -> bool {
    !Excluded::names(spec.name()) && spec.code_mode()
}

/// One pool entry per eligible, launchable server in `registry`.
pub fn pool_from_registry(registry: &McpRegistry, dir: &str, session: Option<&str>) -> Pool {
    let at = registry.bind(dir, session);
    let mut taken = HashSet::new();
    let mut entries = Vec::new();
    for spec in registry.all().filter(|spec| eligible(spec)) {
        // Remote servers come back as `opman mcp-proxy <name>`, so credentials stay in opman.
        let Some(wire) = spec.stdio_launch(at) else {
            continue;
        };
        let launch = Launch::from(&wire);
        let namespace = Namespace::for_server(spec.name(), &taken);
        taken.insert(namespace.clone());
        let open: Opener = Box::new(move || {
            let launch = launch.clone();
            Box::pin(async move { launch.open().await })
        });
        entries.push(Entry::new(namespace, spec.name(), open));
    }
    Pool::new(entries)
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod select_tests;
