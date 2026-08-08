//! Stopping, finding and waiting for an agent.

use super::*;
use crate::mcp_agent_manager::fake_runner::{Harness, DIR};
use crate::mcp_agent_manager::queue::QueuedMessage;

/// A transcript in the shape `progress` hands over: newest first, reasoning alongside the
/// answer.
fn transcript() -> Value {
    json!([
        { "info": { "role": "assistant" }, "parts": [
            { "type": "reasoning", "text": "the user wants a single word" },
            { "type": "text", "text": "LUNA-OK" },
        ]},
        { "info": { "role": "user" }, "parts": [{ "type": "text", "text": "reply LUNA-OK" }] },
    ])
}

async fn session(harness: &Harness) -> String {
    harness
        .state
        .registry
        .create_session(crate::runner::RunnerKind::Opencode, DIR, "child")
        .await
        .expect("create")
        .id
}

#[tokio::test]
async fn aborting_reaches_the_runner_that_owns_the_session() {
    let harness = Harness::new();
    let target = session(&harness).await;

    let result = abort(&harness.state, &target, DIR).await.expect("aborted");

    assert_eq!(result["aborted"], true);
    assert_eq!(harness.opencode.aborted().await, vec![target]);
    assert!(
        harness.claude.aborted().await.is_empty(),
        "the other runner must not be asked"
    );
}

/// The default runner's sessions are the ones an agent is most likely to want, and the
/// registry's own `sessions` deliberately omits them — the web state gets them elsewhere.
/// An agent has no elsewhere.
#[tokio::test]
async fn listing_includes_the_default_runners_own_sessions() {
    let harness = Harness::new();
    let on_default = session(&harness).await;
    let on_other = harness
        .state
        .registry
        .create_session(crate::runner::RunnerKind::Claude, DIR, "peer")
        .await
        .expect("create")
        .id;

    let listed = list(&harness.state, DIR).await.expect("listed");

    assert_eq!(listed["count"], 2);
    let ids: Vec<&str> = listed["agents"]
        .as_array()
        .map(|agents| {
            agents
                .iter()
                .filter_map(|agent| agent["agent_id"].as_str())
                .collect()
        })
        .unwrap_or_default();
    assert!(ids.contains(&on_default.as_str()), "{ids:?}");
    assert!(ids.contains(&on_other.as_str()), "{ids:?}");
}

#[tokio::test]
async fn listing_reports_busy_state_and_queue_depth_per_agent() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness.opencode.set_busy(true);
    harness
        .state
        .queues
        .lock()
        .await
        .entry(target.clone())
        .or_default()
        .push(QueuedMessage {
            id: "msg_1".into(),
            source: String::new(),
            target: target.clone(),
            directory: DIR.into(),
            runner: None,
            body: json!({}),
        });

    let listed = list(&harness.state, DIR).await.expect("listed");

    let agent = &listed["agents"][0];
    assert_eq!(agent["busy"], true);
    assert_eq!(agent["queued_messages"], 1);
    assert_eq!(agent["runner"], "opencode");
}

#[tokio::test(start_paused = true)]
async fn waiting_returns_the_reply_once_the_turn_finishes() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness.opencode.set_busy(true);
    harness.opencode.set_transcript(transcript()).await;
    let runner = harness.opencode.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        runner.set_busy(false);
    });

    let result = wait(&harness.state, &target, DIR, Some(60))
        .await
        .expect("waited");

    assert_eq!(result["timed_out"], false);
    assert_eq!(result["busy"], false);
    // The answer, not the thinking that preceded it.
    assert_eq!(result["reply"], "LUNA-OK");
}

/// Running out of patience is not an error — a slow turn is still a turn, and the caller
/// wants what there is so far.
#[tokio::test(start_paused = true)]
async fn waiting_past_the_timeout_reports_it_rather_than_failing() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness.opencode.set_busy(true);
    harness.opencode.set_transcript(transcript()).await;

    let result = wait(&harness.state, &target, DIR, Some(5))
        .await
        .expect("a timeout is not an error");

    assert_eq!(result["timed_out"], true);
    assert_eq!(result["busy"], true);
}

/// A dispatch is asynchronous, so a wait issued right after a send routinely arrives
/// before the target has picked the message up. Returning immediately there would hand
/// back the *previous* turn's reply as this one's.
#[tokio::test(start_paused = true)]
async fn waiting_on_an_idle_target_settles_before_giving_up_on_a_turn_starting() {
    let harness = Harness::new();
    let target = session(&harness).await;
    harness.opencode.set_transcript(transcript()).await;
    let runner = harness.opencode.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        runner.set_busy(true);
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        runner.set_busy(false);
    });

    let started = tokio::time::Instant::now();
    let result = wait(&harness.state, &target, DIR, Some(60))
        .await
        .expect("waited");

    assert_eq!(result["timed_out"], false);
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(5),
        "the wait returned before the turn it was waiting for had run",
    );
}

#[test]
fn a_transcript_with_no_assistant_answer_yields_an_empty_reply() {
    let only_user = json!({ "messages": [
        { "info": { "role": "user" }, "parts": [{ "type": "text", "text": "hi" }] },
    ]});
    assert_eq!(last_assistant_text(&only_user), "");
    assert_eq!(last_assistant_text(&json!({})), "");

    // An assistant turn that produced only reasoning is not an answer either.
    let thinking = json!({ "messages": [
        { "info": { "role": "assistant" }, "parts": [{ "type": "reasoning", "text": "hmm" }] },
    ]});
    assert_eq!(last_assistant_text(&thinking), "");
}
