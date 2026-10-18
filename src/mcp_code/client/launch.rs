//! Starting a server as a child process.

use std::process::Stdio;

use tokio::process::Command;

use super::conn::{CallError, Connection};
use crate::mcp_registry::bind::WireStdio;

/// An owned copy of a bound launch, so a pool entry can outlive the registry borrow.
#[derive(Clone, Debug, PartialEq)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
}

impl From<&WireStdio<'_>> for Launch {
    fn from(wire: &WireStdio<'_>) -> Self {
        Self {
            command: wire.command.to_string(),
            args: wire.args.iter().map(|arg| arg.to_string()).collect(),
            env: wire
                .env
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.to_string()))
                .collect(),
            cwd: wire.cwd.as_ref().map(|cwd| cwd.to_string()),
        }
    }
}

impl Launch {
    /// Spawn the child and finish the MCP handshake.
    pub async fn open(&self) -> Result<Connection, String> {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Nobody drains a stderr pipe, and a chatty server would block on it.
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start `{}`: {error}", self.command))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err("the server has no stdio pipes".into());
        };
        Connection::open(stdout, stdin, Some(child))
            .await
            .map_err(|error: CallError| format!("handshake failed: {error}"))
    }
}
