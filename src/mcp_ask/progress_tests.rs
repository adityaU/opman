use super::*;

#[test]
fn progress_strictly_increases_and_omits_an_unknown_total() {
    let token = json!("tok-1");
    let first = notification(&token, 1);
    let second = notification(&token, 2);

    assert_eq!(first["method"], "notifications/progress");
    assert_eq!(first["params"]["progressToken"], "tok-1");
    assert_eq!(first["params"]["progress"], 1);
    assert_eq!(second["params"]["progress"], 2);
    // The wait is open-ended, so claiming a total would be a lie the client renders.
    assert!(first["params"].get("total").is_none());
    assert!(first["params"]["message"]
        .as_str()
        .is_some_and(|m| m.contains("waiting for the user")));
}

/// A numeric token round-trips as a number: MCP allows either, and rewriting it as a string
/// would leave the client unable to match the notification to its call.
#[test]
fn a_numeric_token_is_echoed_unchanged() {
    assert_eq!(notification(&json!(7), 1)["params"]["progressToken"], 7);
}

#[tokio::test(start_paused = true)]
async fn ticks_arrive_on_the_clock_and_stop_when_dropped() {
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    let ticker = tick_until_dropped(&out, Some(json!("tok-1")));

    // Race against a deadline that outlives two ticks, then drop the ticker.
    tokio::select! {
        never = ticker => match never {},
        () = tokio::time::sleep(TICK * 2 + Duration::from_secs(1)) => {}
    }

    let written = String::from_utf8(out.lock().await.clone()).expect("utf8");
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.len(), 2, "got: {written}");
    let first: serde_json::Value = serde_json::from_str(lines[0]).expect("json");
    assert_eq!(first["params"]["progress"], 1);

    // Dropped: nothing more is written however long we wait.
    tokio::time::sleep(TICK * 3).await;
    assert_eq!(out.lock().await.len(), written.len());
}

/// Progress is only legal for a token the client supplied, so without one the wait is
/// silent rather than inventing a token the client would ignore.
#[tokio::test(start_paused = true)]
async fn no_progress_token_means_no_notifications() {
    let out = Arc::new(Mutex::new(Vec::<u8>::new()));
    tokio::select! {
        never = tick_until_dropped(&out, None) => match never {},
        () = tokio::time::sleep(TICK * 3) => {}
    }
    assert!(out.lock().await.is_empty());
}
