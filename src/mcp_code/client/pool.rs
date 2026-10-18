//! Lazily opened connections, one per eligible server.

use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::conn::Connection;
use super::frame::output_of;
use crate::mcp_code::catalog::{Catalog, Group, Unavailable};
use crate::mcp_code::names::Namespace;
use crate::mcp_code::sandbox::ToolOutput;
use crate::mcp_probe::ToolDef;

/// How long one server gets to start and list its tools when a catalog is built.
const LIST_TIMEOUT: Duration = Duration::from_secs(12);
/// How long a tool may stay silent. Progress notifications restart it.
const CALL_IDLE: Duration = Duration::from_secs(120);
/// Pages of `tools/list` followed per server.
const MAX_PAGES: usize = 20;

/// Opens a connection. Boxed so tests can hand out in-memory ones.
pub type Opener = Box<dyn Fn() -> BoxFuture<'static, Result<Connection, String>> + Send + Sync>;

enum State {
    Idle,
    Ready {
        conn: Arc<Connection>,
        tools: Option<Arc<[ToolDef]>>,
    },
    /// Opening failed. Sticky for the process: a broken server is not retried per call.
    Failed(String),
}

pub struct Entry {
    pub namespace: Namespace,
    pub server: Box<str>,
    open: Opener,
    state: Mutex<State>,
}

impl Entry {
    pub fn new(namespace: Namespace, server: impl Into<Box<str>>, open: Opener) -> Self {
        Self {
            namespace,
            server: server.into(),
            open,
            state: Mutex::new(State::Idle),
        }
    }

    /// The live connection, opening (or reopening a dead one) on demand.
    async fn connection(&self) -> Result<Arc<Connection>, String> {
        let mut state = self.state.lock().await;
        match &*state {
            State::Ready { conn, .. } if conn.is_alive() => return Ok(conn.clone()),
            State::Failed(reason) => return Err(reason.clone()),
            _ => {}
        }
        match (self.open)().await {
            Ok(conn) => {
                let conn = Arc::new(conn);
                *state = State::Ready {
                    conn: conn.clone(),
                    tools: None,
                };
                Ok(conn)
            }
            Err(reason) => {
                *state = State::Failed(reason.clone());
                Err(reason)
            }
        }
    }

    /// The tool list, cached until the server says it changed.
    async fn tools(&self) -> Result<Arc<[ToolDef]>, String> {
        let conn = self.connection().await?;
        if !conn.take_stale() {
            if let State::Ready {
                tools: Some(tools), ..
            } = &*self.state.lock().await
            {
                return Ok(tools.clone());
            }
        }
        let listed: Arc<[ToolDef]> = list_all(&conn).await?.into();
        if let State::Ready { tools, .. } = &mut *self.state.lock().await {
            *tools = Some(listed.clone());
        }
        Ok(listed)
    }
}

async fn list_all(conn: &Connection) -> Result<Vec<ToolDef>, String> {
    let mut all = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let params = match &cursor {
            Some(cursor) => json!({ "cursor": cursor }),
            None => json!({}),
        };
        let page = conn
            .request("tools/list", params, CALL_IDLE)
            .await
            .map_err(|error| format!("tools/list failed: {error}"))?;
        if let Some(Value::Array(entries)) = page.get("tools") {
            // A malformed entry drops itself, not the listing.
            all.extend(
                entries
                    .iter()
                    .cloned()
                    .filter_map(|e| serde_json::from_value(e).ok()),
            );
        }
        cursor = page
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_string);
        if cursor.is_none() {
            break;
        }
    }
    Ok(all)
}

pub struct Pool {
    entries: Vec<Entry>,
}

impl Pool {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    #[cfg(test)]
    pub fn namespaces(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.namespace.as_str()).collect()
    }

    /// Every reachable server's tools. Servers are started here if they were not yet,
    /// all at once; one that is slow or broken lands in `unavailable` instead.
    pub async fn catalog(&self) -> Catalog {
        let listings = futures::future::join_all(self.entries.iter().map(|entry| async move {
            let listed = tokio::time::timeout(LIST_TIMEOUT, entry.tools()).await;
            (entry, listed)
        }))
        .await;
        let mut catalog = Catalog::default();
        for (entry, listed) in listings {
            match listed {
                Ok(Ok(tools)) => catalog.groups.push(Group {
                    namespace: entry.namespace.clone(),
                    tools,
                }),
                Ok(Err(reason)) => catalog.unavailable.push(Unavailable {
                    server: entry.server.clone(),
                    reason,
                }),
                Err(_) => catalog.unavailable.push(Unavailable {
                    server: entry.server.clone(),
                    reason: format!("did not answer within {}s", LIST_TIMEOUT.as_secs()),
                }),
            }
        }
        catalog.groups.sort_by(|a, b| a.namespace.cmp(&b.namespace));
        catalog
    }

    pub async fn call(
        &self,
        namespace: &str,
        tool: &str,
        input: Value,
    ) -> Result<ToolOutput, String> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.namespace.as_str() == namespace)
            .ok_or_else(|| format!("no server is mounted as `{namespace}`"))?;
        let conn = entry.connection().await?;
        let params = json!({ "name": tool, "arguments": input });
        let result = conn
            .request("tools/call", params, CALL_IDLE)
            .await
            .map_err(|error| error.to_string())?;
        output_of(&result)
    }
}

#[cfg(test)]
#[path = "pool_tests.rs"]
mod pool_tests;
