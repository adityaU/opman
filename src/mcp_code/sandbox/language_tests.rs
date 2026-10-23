use super::sandbox_tests::{exec, failure, quick, value};
use super::*;
use std::time::Duration;

#[tokio::test]
async fn unknown_names_fail_loudly_with_the_hosts_message() {
    let report = exec("tools.nope", 0, quick()).await;
    assert!(
        failure(&report).contains("There is no `nope` namespace."),
        "{:?}",
        report.finish
    );
    let report = exec("tools.m.nope", 0, quick()).await;
    assert!(
        failure(&report).contains("`m` has no tool named `nope`"),
        "{:?}",
        report.finish
    );
    let report = exec("tools.$codemode.other", 0, quick()).await;
    assert!(
        failure(&report).contains("only has `search"),
        "{:?}",
        report.finish
    );
}

#[tokio::test]
async fn namespaces_and_tools_can_be_listed() {
    let report = exec(
        "return [Object.keys(tools), Object.keys(tools.m)]",
        0,
        quick(),
    )
    .await;
    let parsed: serde_json::Value = serde_json::from_str(value(&report)).expect("json");
    assert_eq!(parsed[0], serde_json::json!(["m", "$codemode"]));
    assert_eq!(parsed[1][0], "echo");
}

#[tokio::test]
async fn search_goes_through_the_host() {
    let report = exec(
        "return await tools.$codemode.search({query: 'hello'})",
        0,
        quick(),
    )
    .await;
    assert!(value(&report).contains("tools.m.echo"));
    assert!(value(&report).contains("hello"));
}

#[tokio::test]
async fn errors_report_name_message_and_line() {
    let report = exec("\n\nnull.x", 0, quick()).await;
    assert_eq!(
        failure(&report),
        "TypeError: cannot read property 'x' of null (line 3)"
    );
    let report = exec("return )", 0, quick()).await;
    assert!(
        failure(&report).starts_with("SyntaxError:"),
        "{:?}",
        report.finish
    );
    let report = exec("throw 'plain'", 0, quick()).await;
    assert_eq!(failure(&report), "Uncaught: plain");
}

#[tokio::test]
async fn the_call_budget_is_enforced() {
    let limits = Limits {
        max_calls: 3,
        ..quick()
    };
    let report = exec(
        "for (let i = 0; i < 5; i++) await tools.m.echo({}); return 1",
        0,
        limits,
    )
    .await;
    assert!(
        failure(&report).contains("more than 3 tool calls"),
        "{:?}",
        report.finish
    );
}

#[tokio::test]
async fn bad_input_json_is_a_script_error() {
    let report = exec("await tools.m.echo(1n)", 0, quick()).await;
    assert!(matches!(report.finish, Finish::Failed(_)));
}

#[tokio::test]
async fn set_timeout_works_and_is_bounded_by_the_run() {
    let report = exec(
        "await new Promise((done) => setTimeout(done, 50)); return 'ok'",
        0,
        quick(),
    )
    .await;
    assert_eq!(value(&report), "ok");
}

#[tokio::test]
async fn attachments_are_collected() {
    let report = exec("return await tools.m.image({})", 0, quick()).await;
    assert_eq!(report.attachments.len(), 1);
    assert_eq!(report.attachments[0].mime, "image/png");
}

#[tokio::test]
async fn timeout_request_is_clamped() {
    let long = Limits::default().with_timeout_ms(Some(10_000_000));
    assert_eq!(long.timeout, Duration::from_secs(600));
    let short = Limits::default().with_timeout_ms(Some(1));
    assert_eq!(short.timeout, Duration::from_millis(100));
    assert_eq!(
        Limits::default().with_timeout_ms(None).timeout,
        Duration::from_secs(60)
    );
}

#[tokio::test]
async fn search_accepts_string_shorthand() {
    let report = exec("return await tools.$codemode.search('hello')", 0, quick()).await;
    assert!(value(&report).contains("tools.m.echo"));
}
