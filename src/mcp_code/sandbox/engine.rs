//! Building the QuickJS runtime and running one script in it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rquickjs::function::Async;
use rquickjs::{
    context::EvalOptions, AsyncContext, AsyncRuntime, CatchResultExt, Ctx, Function, Object,
    Promise, Value,
};

use super::error::{Failure, SCRIPT_NAME};
use super::host::ToolHost;
use super::limits::{Cancel, Limits};
use super::state::{Report, RunState};
use super::Finish;
use crate::mcp_code::names::CODEMODE;

const PRELUDE: &str = include_str!("prelude.js");
/// `setTimeout` waits are capped here; the run's own timeout is the real bound.
const MAX_SLEEP_MS: f64 = 600_000.0;

/// Run `code` as the body of an async function and report how it went.
///
/// Three things can end a run early and all are enforced here: the engine's interrupt
/// hook (busy loops), the outer timer (a script parked on a slow tool), and `cancel`.
pub async fn run(code: &str, host: Arc<dyn ToolHost>, limits: Limits, cancel: Cancel) -> Report {
    let started = Instant::now();
    let state = Arc::new(RunState::new(host, limits));
    let work = execute(code, state.clone(), limits, cancel.clone(), started);
    let finish = tokio::select! {
        finish = work => finish,
        () = tokio::time::sleep(limits.timeout) => Finish::TimedOut,
        () = cancel.cancelled() => Finish::Cancelled,
    };
    let (calls, logs, attachments) = state.into_parts();
    Report {
        finish,
        logs,
        calls,
        attachments,
        elapsed: started.elapsed(),
    }
}

async fn execute(
    code: &str,
    state: Arc<RunState>,
    limits: Limits,
    cancel: Cancel,
    started: Instant,
) -> Finish {
    let runtime = match AsyncRuntime::new() {
        Ok(runtime) => runtime,
        Err(error) => return Finish::Failed(Failure::internal(error)),
    };
    runtime.set_memory_limit(limits.memory).await;
    runtime.set_max_stack_size(limits.stack).await;
    let deadline = started + limits.timeout;
    let stop = cancel.clone();
    let hook = move || stop.is_cancelled() || Instant::now() >= deadline;
    runtime.set_interrupt_handler(Some(Box::new(hook))).await;
    let context = match AsyncContext::full(&runtime).await {
        Ok(context) => context,
        Err(error) => return Finish::Failed(Failure::internal(error)),
    };
    let finish = context
        .async_with(async |ctx| in_context(&ctx, code, &state, &cancel).await)
        .await;
    if cancel.is_cancelled() {
        return Finish::Cancelled;
    }
    if Instant::now() >= deadline {
        return Finish::TimedOut;
    }
    finish
}

async fn in_context<'js>(
    ctx: &Ctx<'js>,
    code: &str,
    state: &Arc<RunState>,
    cancel: &Cancel,
) -> Finish {
    let render = match install(ctx, state) {
        Ok(render) => render,
        Err(error) => return Finish::Failed(Failure::internal(error)),
    };
    // One line, so the script's line numbers are the model's own.
    let source = format!("(async () => {{{code}\n}})()");
    let promise: Promise = match ctx
        .eval_with_options(source, options(SCRIPT_NAME))
        .catch(ctx)
    {
        Ok(promise) => promise,
        Err(caught) => return Finish::Failed(Failure::from_caught(caught, &render)),
    };
    let settled: Result<Value, _> = promise.into_future::<Value>().await.catch(ctx);
    match settled {
        Ok(value) => match render.call::<_, String>((value,)).catch(ctx) {
            Ok(text) => Finish::Value(text),
            Err(caught) => Finish::Failed(Failure::from_caught(caught, &render)),
        },
        Err(caught) => {
            let failure = Failure::from_caught(caught, &render);
            if failure.message == "interrupted" {
                return if cancel.is_cancelled() {
                    Finish::Cancelled
                } else {
                    Finish::TimedOut
                };
            }
            Finish::Failed(failure)
        }
    }
}

fn options(name: &str) -> EvalOptions {
    let mut options = EvalOptions::default();
    options.filename = Some(name.to_string());
    options
}

/// Build the host bridge, run the prelude, and return its "print the result" function.
fn install<'js>(ctx: &Ctx<'js>, state: &Arc<RunState>) -> rquickjs::Result<Function<'js>> {
    let host = Object::new(ctx.clone())?;

    let s = state.clone();
    host.set(
        "log",
        Function::new(ctx.clone(), move |level: String, text: String| {
            s.log(&level, &text)
        })?,
    )?;

    let s = state.clone();
    host.set(
        "namespaces",
        Function::new(ctx.clone(), move || namespaces_json(s.host()))?,
    )?;

    let s = state.clone();
    host.set(
        "unknown",
        Function::new(ctx.clone(), move |label: String, key: String| {
            unknown_message(s.host(), &label, &key)
        })?,
    )?;

    let s = state.clone();
    host.set(
        "search",
        Function::new(ctx.clone(), move |query: String| {
            s.host().search(&query).to_string()
        })?,
    )?;

    host.set(
        "sleep",
        Function::new(
            ctx.clone(),
            Async(|ms: f64| async move {
                let ms = if ms.is_finite() {
                    ms.clamp(0.0, MAX_SLEEP_MS)
                } else {
                    0.0
                };
                tokio::time::sleep(Duration::from_millis(ms as u64)).await;
            }),
        )?,
    )?;

    let s = state.clone();
    host.set(
        "call",
        Function::new(
            ctx.clone(),
            Async(move |namespace: String, tool: String, input: String| {
                let state = s.clone();
                async move { state.call(&namespace, &tool, &input).await }
            }),
        )?,
    )?;

    let prelude: Function = ctx.eval_with_options(PRELUDE, options("prelude"))?;
    prelude.call((host,))
}

fn namespaces_json(host: &dyn ToolHost) -> String {
    let groups: Vec<serde_json::Value> = host
        .namespaces()
        .into_iter()
        .map(|(ns, tools)| serde_json::json!({ "ns": ns, "tools": tools }))
        .collect();
    serde_json::Value::Array(groups).to_string()
}

/// `label` is the namespace being read from, or empty for `tools` itself.
fn unknown_message(host: &dyn ToolHost, label: &str, key: &str) -> String {
    if label.is_empty() {
        return host.unknown(key, None);
    }
    if label == CODEMODE {
        return format!("`tools.{CODEMODE}` only has `search({{ query }})`, not `{key}`.");
    }
    host.unknown(label, Some(key))
}
