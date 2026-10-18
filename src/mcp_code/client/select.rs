//! Which registry servers the sandbox may reach.

use crate::mcp_registry::spec::ServerSpec;
use crate::mcp_registry::McpRegistry;

use super::launch::Launch;
use super::pool::{Entry, Opener, Pool};
use crate::mcp_code::names::Namespace;

use std::collections::HashSet;

/// Servers that never appear in the sandbox, as a closed set.
///
/// `Code` would recurse. The others act on the user's live session in the UI — asking a
/// question, rendering a card, spawning agents, moving a board task — which a script
/// running unattended must not do on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Excluded {
    Code,
    Ask,
    Ui,
    AgentManager,
    Kanban,
}

impl Excluded {
    pub const ALL: [Self; 5] = [
        Self::Code,
        Self::Ask,
        Self::Ui,
        Self::AgentManager,
        Self::Kanban,
    ];

    pub const fn server(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Ask => "ask",
            Self::Ui => "ui",
            Self::AgentManager => "agent-manager",
            Self::Kanban => "kanban",
        }
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
