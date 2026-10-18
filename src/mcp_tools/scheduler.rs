//! Scheduler tools use the running app's routine store and scheduler.

use super::output::{ToolError, ToolOutput, ToolResult};
use super::workspace::Workspace;
use crate::loopback::Loopback;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;

const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
static CLIENT: OnceLock<Result<reqwest::Client, reqwest::Error>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerTool {
    List,
    Create,
    Update,
    Delete,
    Run,
}

impl SchedulerTool {
    pub const ALL: [Self; 5] = [
        Self::List,
        Self::Create,
        Self::Update,
        Self::Delete,
        Self::Run,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::List => "scheduler_list",
            Self::Create => "scheduler_create",
            Self::Update => "scheduler_update",
            Self::Delete => "scheduler_delete",
            Self::Run => "scheduler_run",
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Run => "run",
        }
    }

    pub fn definition(self) -> Value {
        super::scheduler_schema::definition(self)
    }

    pub async fn call(self, workspace: &Workspace, arguments: Value) -> ToolResult {
        self.call_with_loopback(workspace, arguments, Loopback::load())
            .await
    }

    pub(super) async fn call_with_loopback(
        self,
        workspace: &Workspace,
        arguments: Value,
        loopback: Option<Loopback>,
    ) -> ToolResult {
        let arguments = normalize_arguments(arguments)?;
        let Some(mut loopback) = loopback else {
            return Err(ToolError::new(
                "scheduler needs the running opman app; start opman with its web server, then retry",
            ));
        };
        let client = CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(35))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()
        });
        loopback.client = client
            .as_ref()
            .map_err(|error| ToolError::new(format!("cannot start scheduler client: {error}")))?
            .clone();
        let mut response = loopback
            .post("/internal/scheduler")
            .json(&json!({
                "operation": self.operation(),
                "project_dir": workspace.root(),
                "args": arguments,
            }))
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() && self != Self::List {
                    return ToolError::new(format!(
                        "scheduler {} timed out; it may have completed. Check scheduler_list before retrying",
                        self.operation()
                    ));
                }
                ToolError::new(format!(
                    "cannot reach the opman scheduler: {error}; check that opman is running"
                ))
            })?;
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| ToolError::new(format!("cannot read scheduler reply: {error}")))?
        {
            if bytes.len().saturating_add(chunk.len()) > RESPONSE_LIMIT {
                return Err(ToolError::new("scheduler reply exceeds 2 MiB"));
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            let reason = error_reason(&bytes);
            return Err(ToolError::new(format!(
                "scheduler {} failed (HTTP {status}): {reason}",
                self.operation()
            )));
        }
        let data: Value = serde_json::from_slice(&bytes)
            .map_err(|error| ToolError::new(format!("invalid scheduler reply: {error}")))?;
        if !data.is_object() {
            return Err(ToolError::new(
                "invalid scheduler reply: expected a JSON object",
            ));
        }
        ToolOutput::ok(format!("{}: {data}", self.name()), &data)
    }
}

fn normalize_arguments(arguments: Value) -> Result<Value, ToolError> {
    let arguments = match arguments {
        Value::Null => serde_json::Map::new(),
        Value::Object(arguments) => arguments,
        _ => {
            return Err(ToolError::new(
                "invalid scheduler arguments: expected an object",
            ))
        }
    };
    Ok(Value::Object(arguments))
}

fn error_reason(bytes: &[u8]) -> String {
    if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        if let Some(reason) = value.get("error").and_then(Value::as_str) {
            return reason.chars().take(2000).collect();
        }
    }
    String::from_utf8_lossy(bytes).chars().take(2000).collect()
}
