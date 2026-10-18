use super::bash::{run, BashArgs};
use super::workspace::Workspace;
use serde_json::json;
use std::time::{Duration, Instant};

fn args(value: serde_json::Value) -> BashArgs {
    serde_json::from_value(value).expect("test fixture or assertion")
}

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("test fixture or assertion");
    let ws = Workspace::new(dir.path().to_path_buf());
    (dir, ws)
}

#[tokio::test]
async fn runs_in_the_project_and_reports_output_and_exit_code() {
    let (dir, ws) = ws();
    std::fs::write(dir.path().join("a.txt"), "x").expect("test fixture or assertion");
    let out = run(&ws, args(json!({"command": "ls; echo oops >&2; exit 3"})))
        .await
        .expect("test fixture or assertion");
    assert!(!out.is_error);
    assert_eq!(out.data["exit_code"], 3);
    assert_eq!(out.data["timed_out"], false);
    assert!(out.data["stdout"]
        .as_str()
        .expect("test fixture or assertion")
        .contains("a.txt"));
    assert_eq!(out.data["stderr"], "oops\n");
    assert!(out.text.contains("[stderr]\noops"));
    assert!(out.text.ends_with("[exit code 3]"));
}

#[tokio::test]
async fn cwd_is_resolved_against_the_project() {
    let (dir, ws) = ws();
    std::fs::create_dir(dir.path().join("sub")).expect("test fixture or assertion");
    let out = run(&ws, args(json!({"command": "pwd", "cwd": "sub"})))
        .await
        .expect("test fixture or assertion");
    assert!(out.data["stdout"]
        .as_str()
        .expect("test fixture or assertion")
        .trim_end()
        .ends_with("/sub"));
    let missing = run(&ws, args(json!({"command": "pwd", "cwd": "nope"}))).await;
    assert!(missing
        .err()
        .expect("test fixture or assertion")
        .message()
        .contains("not a directory"));
}

#[tokio::test]
async fn empty_command_is_an_error() {
    let (_dir, ws) = ws();
    assert!(run(&ws, args(json!({"command": "  "}))).await.is_err());
}

#[tokio::test]
async fn timeout_kills_the_whole_process_group() {
    let (dir, ws) = ws();
    let marker = dir.path().join("alive");
    // A background child that would write the marker after the timeout, if it survived.
    let script = format!("(sleep 1.5; touch {}) & sleep 30", marker.display());
    let started = Instant::now();
    let out = run(&ws, args(json!({"command": script, "timeout_ms": 300})))
        .await
        .expect("test fixture or assertion");
    assert!(out.is_error);
    assert_eq!(out.data["timed_out"], true);
    assert!(out.text.contains("timed out after 300 ms"));
    assert!(started.elapsed() < Duration::from_secs(5));
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert!(!marker.exists(), "child survived the kill");
}

#[tokio::test]
async fn big_output_is_cut_in_the_middle_and_says_so() {
    let (_dir, ws) = ws();
    let out = run(&ws, args(json!({"command": "seq 1 20000"})))
        .await
        .expect("test fixture or assertion");
    let stdout = out.data["stdout"]
        .as_str()
        .expect("test fixture or assertion");
    assert!(stdout.len() < 31_000);
    assert!(stdout.starts_with("1\n2\n"));
    assert!(stdout.trim_end().ends_with("20000"));
    assert!(stdout.contains("bytes omitted"));
    assert!(
        out.data["stdout_omitted_bytes"]
            .as_u64()
            .expect("test fixture or assertion")
            > 0
    );
}

#[tokio::test]
async fn dropping_the_call_kills_the_command() {
    let (dir, ws) = ws();
    let marker = dir.path().join("alive");
    let script = format!("sleep 1; touch {}", marker.display());
    let call = run(&ws, args(json!({"command": script})));
    let _ = tokio::time::timeout(Duration::from_millis(300), call).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        !marker.exists(),
        "command kept running after the call was dropped"
    );
}
