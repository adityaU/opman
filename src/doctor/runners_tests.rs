use super::*;
use crate::doctor::report::Status;
use std::os::unix::fs::PermissionsExt;

fn script(dir: &Path, body: &str) -> std::path::PathBuf {
    let file = dir.join("fake-cli");
    std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    file
}

#[tokio::test]
async fn a_working_cli_reports_its_first_line() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bin = script(dir.path(), "echo '1.2.3 (Fake)'; echo more");
    let check = version("fake", &bin, LIMIT).await;
    assert_eq!(check.status, Status::Pass);
    assert_eq!(check.title, "fake version: 1.2.3 (Fake)");
}

#[tokio::test]
async fn a_failing_cli_shows_its_stderr() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bin = script(dir.path(), "echo 'bad config' >&2; exit 2");
    let check = version("fake", &bin, LIMIT).await;
    assert_eq!(check.status, Status::Fail);
    assert_eq!(check.detail.as_deref(), Some("bad config"));
}

#[tokio::test]
async fn a_hanging_cli_times_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bin = script(dir.path(), "exec sleep 30");
    let check = version("fake", &bin, Duration::from_millis(200)).await;
    assert_eq!(check.status, Status::Fail);
    assert!(check.title.contains("did not finish"));
}
