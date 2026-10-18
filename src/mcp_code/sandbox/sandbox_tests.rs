use std::time::{Duration, Instant};

use super::*;
use crate::mcp_code::testing::FakeHost;

pub(super) async fn exec(code: &str, delay_ms: u64, limits: Limits) -> Report {
    run(code, FakeHost::new(delay_ms), limits, Cancel::default()).await
}

pub(super) fn quick() -> Limits {
    Limits {
        timeout: Duration::from_secs(10),
        ..Limits::default()
    }
}

pub(super) fn value(report: &Report) -> &str {
    match &report.finish {
        Finish::Value(text) => text,
        other => panic!("expected a value, got {other:?}"),
    }
}

pub(super) fn failure(report: &Report) -> String {
    match &report.finish {
        Finish::Failed(failure) => failure.to_string(),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[tokio::test]
async fn returns_strings_raw_and_everything_else_as_json() {
    assert_eq!(value(&exec("return 'hi'", 0, quick()).await), "hi");
    assert_eq!(value(&exec("return 42", 0, quick()).await), "42");
    assert_eq!(
        value(&exec("return {a: [1, 2]}", 0, quick()).await),
        "{\n  \"a\": [\n    1,\n    2\n  ]\n}"
    );
    assert_eq!(value(&exec("", 0, quick()).await), "");
    assert_eq!(value(&exec("return 10n", 0, quick()).await), "\"10n\"");
}

#[tokio::test]
async fn console_output_is_captured_with_levels() {
    let report = exec(
        "console.log('a', {b: 1}); console.warn('w'); console.error(new Error('e')); console.info(); return 1",
        0,
        quick(),
    )
    .await;
    let lines: Vec<(&str, &str)> = report
        .logs
        .iter()
        .map(|l| (l.level.as_str(), l.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            ("log", "a {\"b\":1}"),
            ("warn", "w"),
            ("error", "Error: e"),
            ("info", "")
        ]
    );
}

#[tokio::test]
async fn console_output_is_bounded() {
    let report = exec(
        "for (let i = 0; i < 2000; i++) console.log('x'.repeat(100)); return 1",
        0,
        quick(),
    )
    .await;
    let total: usize = report.logs.iter().map(|l| l.text.len()).sum();
    assert!(total < 20_000, "{total}");
}

#[tokio::test]
async fn there_is_no_filesystem_network_or_process_access() {
    let code = "return [typeof require, typeof fetch, typeof process, typeof XMLHttpRequest, \
                typeof WebSocket, typeof Deno, typeof Bun, typeof std, typeof os, typeof module, \
                typeof readFile].join(',')";
    let report = exec(code, 0, quick()).await;
    assert_eq!(value(&report), "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined");
}

#[tokio::test]
async fn dynamic_import_does_not_load_anything() {
    let report = exec("await import('fs'); return 'loaded'", 0, quick()).await;
    assert!(
        matches!(report.finish, Finish::Failed(_)),
        "{:?}",
        report.finish
    );
}

#[tokio::test]
async fn tools_is_read_only() {
    let report = exec(
        "try { tools.m = 1 } catch (e) { return e.message } return 'wrote'",
        0,
        quick(),
    )
    .await;
    assert_eq!(value(&report), "tools is read-only");
}

#[tokio::test]
async fn infinite_loop_times_out() {
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let started = Instant::now();
    let report = exec("while (true) {}", 0, limits).await;
    assert_eq!(report.finish, Finish::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn memory_bomb_is_stopped() {
    let report = exec(
        "const a = []; while (true) a.push('x'.repeat(1000000));",
        0,
        quick(),
    )
    .await;
    assert!(
        failure(&report).contains("out of memory"),
        "{:?}",
        report.finish
    );
}

#[tokio::test]
async fn runaway_recursion_is_stopped() {
    let report = exec("function f() { return f() + 1 } return f()", 0, quick()).await;
    assert!(failure(&report).contains("stack"), "{:?}", report.finish);
}

#[tokio::test]
async fn a_script_parked_on_a_slow_tool_times_out() {
    let limits = Limits {
        timeout: Duration::from_millis(200),
        ..Limits::default()
    };
    let report = exec("return await tools.m.slow({})", 5_000, limits).await;
    assert_eq!(report.finish, Finish::TimedOut);
    assert_eq!(report.calls.len(), 1);
    assert!(!report.calls[0].finished);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_stops_a_busy_loop_and_a_parked_call() {
    for code in ["while (true) {}", "await tools.m.slow({})"] {
        let cancel = Cancel::default();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            trigger.cancel();
        });
        let started = Instant::now();
        let report = run(code, FakeHost::new(5_000), quick(), cancel).await;
        assert_eq!(report.finish, Finish::Cancelled, "{code}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}

#[tokio::test]
async fn promise_all_runs_calls_at_the_same_time() {
    let started = Instant::now();
    let report = exec(
        "const r = await Promise.all([1, 2, 3, 4].map(() => tools.m.slow({}))); return r.length",
        250,
        quick(),
    )
    .await;
    assert_eq!(value(&report), "4");
    assert!(
        started.elapsed() < Duration::from_millis(700),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(report.calls.len(), 4);
    assert!(report.calls.iter().all(|c| c.ok && c.finished));
}

#[tokio::test]
async fn at_most_parallel_calls_are_in_flight() {
    let limits = Limits {
        parallel: 2,
        ..quick()
    };
    let started = Instant::now();
    exec(
        "await Promise.all([1, 2, 3, 4].map(() => tools.m.slow({})))",
        200,
        limits,
    )
    .await;
    assert!(
        started.elapsed() >= Duration::from_millis(390),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn calls_are_logged_in_start_order_with_input_and_output() {
    let report = exec(
        "await Promise.all([tools.m.echo({n: 1}), tools.m.echo({n: 2}), tools.m.echo({n: 3})])",
        0,
        quick(),
    )
    .await;
    let inputs: Vec<&str> = report.calls.iter().map(|c| c.input.text.as_str()).collect();
    assert_eq!(inputs, ["{\"n\":1}", "{\"n\":2}", "{\"n\":3}"]);
    assert_eq!(report.calls[0].tool, "m.echo");
    assert!(report.calls[0].output.text.contains("\"n\": 1"));
}

#[tokio::test]
async fn tool_errors_become_catchable_errors() {
    let caught = exec(
        "try { await tools.m.fail({}) } catch (e) { return e.message }",
        0,
        quick(),
    )
    .await;
    assert_eq!(value(&caught), "boom");
    assert!(!caught.calls[0].ok);
    assert_eq!(caught.calls[0].output.text, "boom");

    let uncaught = exec("await tools.m.fail({})", 0, quick()).await;
    assert_eq!(failure(&uncaught), "Error: boom (line 1)");
}
