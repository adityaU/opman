use super::bash::{run_after, BashArgs};
use super::limits::Timeout;
use super::workspace::Workspace;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

const AFTER: Timeout = Timeout::from_secs(1);

fn args(value: Value) -> BashArgs {
    serde_json::from_value(value).expect("test fixture or assertion")
}

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("test fixture or assertion");
    let ws = Workspace::new(dir.path().to_path_buf());
    (dir, ws)
}

fn file_of<'a>(data: &'a Value, key: &str) -> &'a Path {
    Path::new(data["job"][key].as_str().expect("test fixture or assertion"))
}

async fn wait_for(path: &Path) -> String {
    for _ in 0..100 {
        if let Ok(text) = std::fs::read_to_string(path) {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{} never appeared", path.display());
}

#[tokio::test]
async fn a_long_command_moves_to_the_background_and_finishes_into_files() {
    let (_dir, ws) = ws();
    let out = run_after(
        &ws,
        args(json!({"command": "echo start; sleep 2; echo end; exit 4"})),
        AFTER,
    )
    .await
    .expect("test fixture or assertion");
    assert!(!out.is_error);
    assert_eq!(out.data["background"], true);
    assert!(out.text.contains("moved to the background"));
    assert!(out.text.contains("tail -n 40"));
    assert!(out.text.contains("`wait` tool"));
    assert!(out.text.contains("every few minutes"));
    let exit = wait_for(file_of(&out.data, "exit_file")).await;
    assert_eq!(exit, "exit code 4\n");
    let stdout = std::fs::read_to_string(file_of(&out.data, "stdout_file"))
        .expect("test fixture or assertion");
    assert_eq!(stdout, "start\nend\n");
}

#[tokio::test]
async fn a_background_job_is_killed_at_a_longer_timeout() {
    let (dir, ws) = ws();
    let marker = dir.path().join("alive");
    let script = format!("(sleep 3; touch {}) & sleep 30", marker.display());
    let out = run_after(
        &ws,
        args(json!({"command": script, "timeout_ms": 1_500})),
        AFTER,
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(out.data["background"], true);
    let exit = wait_for(file_of(&out.data, "exit_file")).await;
    assert!(exit.starts_with("timed out"), "{exit}");
    tokio::time::sleep(Duration::from_millis(3000)).await;
    assert!(!marker.exists(), "child survived the kill");
}

#[tokio::test]
async fn a_short_timeout_still_kills_instead_of_backgrounding() {
    let (_dir, ws) = ws();
    let out = run_after(
        &ws,
        args(json!({"command": "sleep 30", "timeout_ms": 300})),
        AFTER,
    )
    .await
    .expect("test fixture or assertion");
    assert!(out.is_error);
    assert_eq!(out.data["timed_out"], true);
    assert_eq!(out.data["background"], false);
}

#[tokio::test]
async fn a_quick_command_is_not_backgrounded() {
    let (_dir, ws) = ws();
    let out = run_after(&ws, args(json!({"command": "echo hi"})), AFTER)
        .await
        .expect("test fixture or assertion");
    assert_eq!(out.data["background"], false);
    assert!(out.data.get("job").is_none());
    assert_eq!(out.data["stdout"], "hi\n");
}
