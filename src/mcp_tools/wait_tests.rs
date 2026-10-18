use super::*;

fn args(value: Value) -> WaitArgs {
    serde_json::from_value(value).expect("valid wait fixture")
}

#[test]
fn rejects_missing_ambiguous_and_unknown_arguments() {
    for value in [
        json!({}),
        json!({"seconds": 1, "until": "2026-10-18T12:00:00Z"}),
        json!({"seconds": 1, "other": true}),
        json!({"seconds": "1"}),
    ] {
        assert!(serde_json::from_value::<WaitArgs>(value).is_err());
    }
}

#[test]
fn rejects_negative_and_overlong_waits() {
    for value in [json!({"seconds": -1}), json!({"seconds": 601})] {
        assert!(args(value).duration().is_err());
    }
    let too_far = (Utc::now() + chrono::Duration::seconds(700)).to_rfc3339();
    assert!(args(json!({"until": too_far})).duration().is_err());
}

#[test]
fn timestamps_require_a_timezone_and_past_times_complete_immediately() {
    assert!(args(json!({"until": "not a time"})).duration().is_err());
    assert!(args(json!({"until": "2026-10-18T12:00:00"}))
        .duration()
        .is_err());
    assert_eq!(
        args(json!({"until": "2000-01-01T00:00:00+05:30"})).duration(),
        Ok(Duration::ZERO)
    );
    let soon = (Utc::now() + chrono::Duration::seconds(30)).to_rfc3339();
    let delay = args(json!({"until": soon}))
        .duration()
        .expect("future timestamp");
    assert!(delay > Duration::from_secs(28) && delay <= Duration::from_secs(30));
}

#[tokio::test(start_paused = true)]
async fn fractional_wait_yields_until_the_timer_finishes() {
    let future = run(args(json!({"seconds": 2.5})));
    tokio::pin!(future);
    tokio::select! {
        _ = &mut future => panic!("wait completed before its deadline"),
        _ = tokio::time::sleep(Duration::from_secs(1)) => {}
    }
    tokio::time::advance(Duration::from_millis(1500)).await;
    let result = future.await.expect("wait succeeds");
    assert_eq!(result.data["waited_seconds"], 2.5);
    assert!(result.data["completed_at"]
        .as_str()
        .is_some_and(|time| DateTime::parse_from_rfc3339(time).is_ok()));
}

#[tokio::test(start_paused = true)]
async fn abort_stops_a_long_wait_without_advancing_time() {
    let started = Instant::now();
    let task = tokio::spawn(run(args(json!({"seconds": 600}))));
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.is_err_and(|error| error.is_cancelled()));
    assert_eq!(started.elapsed(), Duration::ZERO);
}

#[tokio::test]
async fn zero_wait_returns_structured_completion() {
    let result = run(args(json!({"seconds": 0})))
        .await
        .expect("zero wait succeeds");
    assert!(!result.is_error);
    assert!(result.data["waited_seconds"].is_number());
}
