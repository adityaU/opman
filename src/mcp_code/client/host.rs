//! The sandbox's view of a [`Pool`], frozen at one catalog snapshot.

use std::sync::Arc;

use futures::future::BoxFuture;
use serde_json::Value;

use super::pool::Pool;
use crate::mcp_code::catalog::Catalog;
use crate::mcp_code::names::{access_path, closest, CODEMODE};
use crate::mcp_code::sandbox::{ToolHost, ToolOutput};
use crate::mcp_code::search;

pub struct PoolHost {
    pool: Arc<Pool>,
    catalog: Arc<Catalog>,
}

impl PoolHost {
    pub fn new(pool: Arc<Pool>, catalog: Arc<Catalog>) -> Self {
        Self { pool, catalog }
    }
}

impl ToolHost for PoolHost {
    fn namespaces(&self) -> Vec<(String, Vec<String>)> {
        self.catalog
            .groups
            .iter()
            .map(|group| {
                let names = group.tools.iter().map(|tool| tool.name.clone()).collect();
                (group.namespace.as_str().to_string(), names)
            })
            .collect()
    }

    fn call<'a>(
        &'a self,
        namespace: &'a str,
        tool: &'a str,
        input: Value,
    ) -> BoxFuture<'a, Result<ToolOutput, String>> {
        Box::pin(async move {
            // Checked against the snapshot first, so a typo gets suggestions instead of
            // a bare "unknown tool" from the far end.
            if self.catalog.tool(namespace, tool).is_none() {
                return Err(self.unknown(namespace, Some(tool)));
            }
            self.pool.call(namespace, tool, input).await
        })
    }

    fn search(&self, query: &str) -> Value {
        search::search(&self.catalog, query)
    }

    fn unknown(&self, namespace: &str, tool: Option<&str>) -> String {
        let Some(tool) = tool else {
            return unknown_namespace(&self.catalog, namespace);
        };
        let Some(group) = self.catalog.group(namespace) else {
            return unknown_namespace(&self.catalog, namespace);
        };
        let near = closest(tool, group.tools.iter().map(|t| t.name.as_str()));
        let mut message = format!("`{namespace}` has no tool named `{tool}`.");
        if near.is_empty() {
            message.push_str(&format!(
                " Try `Object.keys(tools.{namespace})` to list them."
            ));
            return message;
        }
        let paths: Vec<String> = near
            .iter()
            .map(|name| access_path(namespace, name))
            .collect();
        message.push_str(&format!(" Did you mean {}?", paths.join(" or ")));
        message
    }
}

fn unknown_namespace(catalog: &Catalog, namespace: &str) -> String {
    let names = catalog
        .groups
        .iter()
        .map(|group| group.namespace.as_str())
        .chain(std::iter::once(CODEMODE));
    let near = closest(namespace, names.clone());
    if near.is_empty() {
        return format!(
            "There is no `{namespace}` namespace. Available: {}.",
            names.collect::<Vec<_>>().join(", ")
        );
    }
    format!(
        "There is no `{namespace}` namespace. Did you mean {}?",
        near.join(" or ")
    )
}
