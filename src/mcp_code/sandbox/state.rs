//! What one run accumulates: tool calls, console lines, attachments.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::Semaphore;

use super::host::{Attachment, ToolHost};
use super::limits::Limits;
use super::output::{truncate, Capped};
use super::Finish;

/// Per input or output kept in the call log.
pub const CALL_TEXT_CAP: usize = 8 * 1024;
/// All call text kept in one run; later calls keep only their names and sizes.
pub const CALL_TOTAL_CAP: usize = 64 * 1024;
/// Console text kept in one run.
const LOG_CAP: usize = 16 * 1024;

/// One tool call, in the order it started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallRecord {
    /// `namespace.tool`.
    pub tool: String,
    pub input: Capped,
    /// The result text on success, or the error text.
    pub output: Capped,
    pub ok: bool,
    pub duration_ms: u64,
    /// False when the run ended before this call answered.
    pub finished: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub level: String,
    pub text: String,
}

/// Everything a finished run has to say.
#[derive(Debug)]
pub struct Report {
    pub finish: Finish,
    pub logs: Vec<LogLine>,
    pub calls: Vec<CallRecord>,
    pub attachments: Vec<Attachment>,
    pub elapsed: Duration,
}

#[derive(Default)]
struct Recorded {
    calls: Vec<CallRecord>,
    call_bytes: usize,
    logs: Vec<LogLine>,
    log_bytes: usize,
    attachments: Vec<Attachment>,
}

pub struct RunState {
    host: Arc<dyn ToolHost>,
    limits: Limits,
    started: AtomicUsize,
    gate: Semaphore,
    recorded: Mutex<Recorded>,
}

impl RunState {
    pub fn new(host: Arc<dyn ToolHost>, limits: Limits) -> Self {
        Self {
            host,
            limits,
            started: AtomicUsize::new(0),
            gate: Semaphore::new(limits.parallel.max(1)),
            recorded: Mutex::new(Recorded::default()),
        }
    }

    pub fn host(&self) -> &dyn ToolHost {
        self.host.as_ref()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Recorded> {
        self.recorded.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn log(&self, level: &str, text: &str) {
        let mut rec = self.lock();
        if rec.log_bytes >= LOG_CAP {
            return;
        }
        let room = LOG_CAP - rec.log_bytes;
        let (kept, cut) = truncate(text, room);
        let mut kept = kept.to_string();
        if cut {
            kept.push_str(" ...[console output truncated]");
        }
        rec.log_bytes += text.len();
        rec.logs.push(LogLine {
            level: level.to_string(),
            text: kept,
        });
    }

    /// Run one tool call for the script and return the JSON the prelude expects:
    /// `{"ok":true,"value":...}` or `{"ok":false,"error":"..."}`.
    pub async fn call(&self, namespace: &str, tool: &str, input_json: &str) -> String {
        let outcome = self.call_inner(namespace, tool, input_json).await;
        match outcome {
            Ok(value) => serde_json::json!({ "ok": true, "value": value }).to_string(),
            Err(error) => serde_json::json!({ "ok": false, "error": error }).to_string(),
        }
    }

    async fn call_inner(
        &self,
        namespace: &str,
        tool: &str,
        input_json: &str,
    ) -> Result<Value, String> {
        if self.started.fetch_add(1, Ordering::Relaxed) >= self.limits.max_calls {
            return Err(format!(
                "This script made more than {} tool calls. Combine work or call fewer tools.",
                self.limits.max_calls
            ));
        }
        let input: Value = serde_json::from_str(input_json).map_err(|error| {
            format!("The input to {namespace}.{tool} is not valid JSON: {error}")
        })?;
        let slot = self.begin(namespace, tool, input_json);
        // The permit is held for the call only; `closed` cannot happen, we never close it.
        let _permit = self
            .gate
            .acquire()
            .await
            .map_err(|_| "tool gate closed".to_string())?;
        let clock = Instant::now();
        let result = self.host.call(namespace, tool, input).await;
        let ms = u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX);
        match result {
            Ok(output) => {
                self.lock().attachments.extend(output.attachments);
                let text = printed(&output.value);
                self.finish(slot, true, &text, ms);
                Ok(output.value)
            }
            Err(error) => {
                self.finish(slot, false, &error, ms);
                Err(error)
            }
        }
    }

    /// Reserve the call's place in start order and keep its (capped) input.
    fn begin(&self, namespace: &str, tool: &str, input_json: &str) -> usize {
        let mut rec = self.lock();
        let input = take_budget(&mut rec.call_bytes, input_json);
        rec.calls.push(CallRecord {
            tool: format!("{namespace}.{tool}"),
            input,
            output: Capped::default(),
            ok: false,
            duration_ms: 0,
            finished: false,
        });
        rec.calls.len() - 1
    }

    fn finish(&self, slot: usize, ok: bool, text: &str, duration_ms: u64) {
        let mut rec = self.lock();
        let output = take_budget(&mut rec.call_bytes, text);
        if let Some(call) = rec.calls.get_mut(slot) {
            call.output = output;
            call.ok = ok;
            call.duration_ms = duration_ms;
            call.finished = true;
        }
    }

    pub fn into_parts(&self) -> (Vec<CallRecord>, Vec<LogLine>, Vec<Attachment>) {
        let mut rec = self.lock();
        (
            std::mem::take(&mut rec.calls),
            std::mem::take(&mut rec.logs),
            std::mem::take(&mut rec.attachments),
        )
    }
}

/// Cap `text` per item and against what is left of the shared budget.
fn take_budget(used: &mut usize, text: &str) -> Capped {
    let left = CALL_TOTAL_CAP.saturating_sub(*used);
    if left == 0 {
        return Capped::omitted(text.len());
    }
    let capped = Capped::new(text, CALL_TEXT_CAP.min(left));
    *used += capped.text.len();
    capped
}

/// Strings raw, everything else pretty JSON: the same rule as the final result.
pub fn printed(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;
