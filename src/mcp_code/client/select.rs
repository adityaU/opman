//! Which registry servers the sandbox may reach.

use crate::mcp_registry::spec::{ServerSpec, Transport};
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

/// Whether `spec` may be mounted. Settings are checked live by the pool.
pub fn eligible(spec: &ServerSpec) -> bool {
    !Excluded::names(spec.name())
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
        let launch = code_mode_launch(spec, &wire);
        let namespace = Namespace::for_server(spec.name(), &taken);
        taken.insert(namespace.clone());
        let open: Opener = Box::new(move || {
            let launch = launch.clone();
            Box::pin(async move { launch.open().await })
        });
        entries.push(Entry::new(namespace, spec.name(), open));
    }
    Pool::with_live_settings(entries)
}

fn code_mode_launch(spec: &ServerSpec, wire: &crate::mcp_registry::bind::WireStdio<'_>) -> Launch {
    let mut launch = Launch::from(wire);
    if matches!(spec.transport(), Transport::Remote(_)) {
        launch.env.push((
            crate::mcp_proxy::CODE_MODE_PROXY_ENV.to_string(),
            "1".to_string(),
        ));
    }
    launch
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod select_tests;
