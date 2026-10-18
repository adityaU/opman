//! A snapshot of every tool the sandbox can reach.

use std::sync::Arc;

use crate::mcp_probe::ToolDef;

use super::names::Namespace;

/// One reachable server and what it listed.
#[derive(Clone, Debug)]
pub struct Group {
    pub namespace: Namespace,
    pub tools: Arc<[ToolDef]>,
}

/// Servers that were declared but could not be listed, with the reason.
#[derive(Clone, Debug)]
pub struct Unavailable {
    pub server: Box<str>,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub groups: Vec<Group>,
    pub unavailable: Vec<Unavailable>,
}

impl Catalog {
    pub fn group(&self, namespace: &str) -> Option<&Group> {
        self.groups
            .iter()
            .find(|group| group.namespace.as_str() == namespace)
    }

    pub fn tool(&self, namespace: &str, tool: &str) -> Option<&ToolDef> {
        self.group(namespace)?
            .tools
            .iter()
            .find(|candidate| candidate.name == tool)
    }

    pub fn tool_count(&self) -> usize {
        self.groups.iter().map(|group| group.tools.len()).sum()
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod catalog_tests;

/// Shared by the describe and search tests.
#[cfg(test)]
pub mod tests_support {
    pub use super::catalog_tests::sample;
}
