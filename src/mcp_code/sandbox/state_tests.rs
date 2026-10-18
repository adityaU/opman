use std::sync::Arc;

use super::*;
use crate::mcp_code::sandbox::Limits;
use crate::mcp_code::testing::FakeHost;

fn state() -> RunState {
    RunState::new(FakeHost::new(0), Limits::default())
}

#[tokio::test]
async fn one_call_is_recorded_with_input_output_and_timing() {
    let state = state();
    let reply = state.call("m", "echo", "{\"a\":1}").await;
    assert_eq!(reply, "{\"ok\":true,\"value\":{\"a\":1}}");
    let (calls, _, _) = state.into_parts();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!(call.tool, "m.echo");
    assert_eq!(call.input.text, "{\"a\":1}");
    assert!(call.output.text.contains("\"a\": 1"));
    assert!(call.ok && call.finished);
}

#[tokio::test]
async fn a_failed_call_records_the_error_text() {
    let state = state();
    let reply = state.call("m", "fail", "{}").await;
    assert_eq!(reply, "{\"error\":\"boom\",\"ok\":false}");
    let (calls, _, _) = state.into_parts();
    assert!(!calls[0].ok);
    assert_eq!(calls[0].output.text, "boom");
}

#[tokio::test]
async fn long_output_is_capped_and_marked() {
    let state = state();
    let reply = state.call("m", "big", "{}").await;
    // The script still gets the whole value; only the log is cut.
    assert!(reply.len() > 20_000);
    let (calls, _, _) = state.into_parts();
    let output = &calls[0].output;
    assert_eq!(output.text.len(), CALL_TEXT_CAP);
    assert!(output.truncated);
    assert_eq!(output.bytes, 20_000);
}

#[tokio::test]
async fn the_total_log_is_bounded_and_later_calls_keep_only_sizes() {
    let state = Arc::new(state());
    for _ in 0..12 {
        state.call("m", "big", "{}").await;
    }
    let (calls, _, _) = state.into_parts();
    let kept: usize = calls
        .iter()
        .map(|c| c.output.text.len() + c.input.text.len())
        .sum();
    assert!(kept <= CALL_TOTAL_CAP, "{kept}");
    let last = calls.last().expect("calls");
    assert!(last.output.text.is_empty() && last.output.truncated);
    assert_eq!(last.output.bytes, 20_000);
}

#[tokio::test]
async fn invalid_input_json_is_an_error_not_a_panic() {
    let reply = state().call("m", "echo", "{nope").await;
    assert!(reply.contains("not valid JSON"), "{reply}");
}

#[test]
fn printed_matches_the_final_result_rule() {
    assert_eq!(printed(&serde_json::json!("raw")), "raw");
    assert_eq!(printed(&serde_json::json!([1])), "[\n  1\n]");
}
