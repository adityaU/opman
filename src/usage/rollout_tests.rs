use super::*;
use crate::usage::fake_provider::{rollout_line, write_rollout};

#[test]
fn parses_a_token_count_event() {
    let snapshot = parse_line(&rollout_line("2026-10-04T16:51:34.608Z", 12.5)).expect("snapshot");
    assert_eq!(snapshot.recorded_at, "2026-10-04T16:51:34Z");
    assert_eq!(snapshot.windows[0].id, "session");
    assert_eq!(snapshot.windows[0].used_percent, 12.5);
    assert_eq!(snapshot.windows[1].label, "Weekly");
    assert_eq!(
        snapshot.credits.and_then(|c| c.balance).as_deref(),
        Some("0")
    );
}

#[test]
fn null_rate_limits_are_skipped() {
    let line = r#"{"timestamp":"2026-10-04T16:51:34Z","payload":{"type":"token_count","rate_limits":null}}"#;
    assert_eq!(parse_line(line), None);
}

#[tokio::test]
async fn newest_log_with_limits_wins() {
    let home = tempfile::tempdir().unwrap();
    write_rollout(
        home.path(),
        "2026/10/03",
        "rollout-2026-10-03T08-00-00-a.jsonl",
        &[rollout_line("2026-10-03T08:00:00Z", 5.0)],
    );
    write_rollout(
        home.path(),
        "2026/10/04",
        "rollout-2026-10-04T09-00-00-b.jsonl",
        &[
            rollout_line("2026-10-04T09:00:00Z", 10.0),
            rollout_line("2026-10-04T09:05:00Z", 20.0),
            "not json".to_string(),
        ],
    );
    // Newest by name, but it recorded no limits (an API-key session).
    write_rollout(
        home.path(),
        "2026/10/04",
        "rollout-2026-10-04T10-00-00-c.jsonl",
        &[r#"{"timestamp":"2026-10-04T10:00:00Z","payload":{"type":"user_message"}}"#.to_string()],
    );
    let snapshot = latest(home.path().to_path_buf()).await.expect("snapshot");
    assert_eq!(snapshot.recorded_at, "2026-10-04T09:05:00Z");
    assert_eq!(snapshot.windows[0].used_percent, 20.0);
}

#[tokio::test]
async fn no_sessions_dir_is_none() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(latest(home.path().to_path_buf()).await, None);
}
