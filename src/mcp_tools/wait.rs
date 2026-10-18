//! A cancellable timer. It yields to Tokio instead of keeping a shell process alive.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::Instant;

use super::output::{ToolError, ToolOutput, ToolResult};
use super::schema::{object, string, tool};

const MAX_WAIT: Duration = Duration::from_secs(600);

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum WaitArgs {
    Seconds(Seconds),
    Until(Until),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seconds {
    seconds: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Until {
    until: String,
}

impl WaitArgs {
    fn duration(self) -> Result<Duration, ToolError> {
        let duration = match self {
            Self::Seconds(args) => Duration::try_from_secs_f64(args.seconds)
                .map_err(|_| ToolError::new("seconds must be a finite, non-negative number"))?,
            Self::Until(args) => DateTime::parse_from_rfc3339(&args.until)
                .map_err(|_| ToolError::new("until must be an RFC3339 timestamp with a timezone"))?
                .signed_duration_since(Utc::now())
                .to_std()
                .unwrap_or(Duration::ZERO),
        };
        if duration > MAX_WAIT {
            return Err(ToolError::new(
                "wait is limited to 600 seconds; use scheduler_create for longer scheduled work",
            ));
        }
        Ok(duration)
    }
}

pub async fn run(args: WaitArgs) -> ToolResult {
    let duration = args.duration()?;
    let started = Instant::now();
    tokio::time::sleep(duration).await;
    let elapsed = started.elapsed().as_secs_f64();
    let completed_at = Utc::now().to_rfc3339();
    ToolOutput::ok(
        format!("Wait finished after {elapsed:.3} seconds."),
        &json!({ "waited_seconds": elapsed, "completed_at": completed_at }),
    )
}

pub fn definition() -> Value {
    let mut input = object(
        vec![
            ("seconds", json!({ "type": "number", "minimum": 0, "maximum": 600,
                "description": "Seconds to wait, including fractions. At most 600." })),
            ("until", string("Wait until this RFC3339 timestamp, including timezone. At most 600 seconds ahead; past times return immediately.")),
        ],
        &[],
    );
    input["oneOf"] = json!([{ "required": ["seconds"] }, { "required": ["until"] }]);
    input["additionalProperties"] = json!(false);
    let mut definition = tool(
        "wait",
        "Pause this tool call without running a shell or blocking other tools. Supply exactly \
         one of seconds or until. Cancellation ends the wait immediately. At most 600 seconds; \
         use scheduler_create for work that should run later without holding this call open.",
        input,
        object(
            vec![
                ("waited_seconds", json!({ "type": "number", "minimum": 0 })),
                ("completed_at", string("Completion time in RFC3339 UTC.")),
            ],
            &["waited_seconds", "completed_at"],
        ),
    );
    definition["annotations"] = json!({
        "readOnlyHint": true, "idempotentHint": true,
        "destructiveHint": false, "openWorldHint": false,
    });
    definition
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
