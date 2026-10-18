use super::*;
use crate::doctor::report::Status;

#[test]
fn reads_the_pid_from_a_socket_name() {
    assert_eq!(socket_pid("opman-agent-manager-1234.sock"), Some(1234));
    assert_eq!(socket_pid("opman-agent-manager-x.sock"), None);
    assert_eq!(socket_pid("other-1234.sock"), None);
    assert_eq!(socket_pid("opman-agent-manager-1234.txt"), None);
}

#[tokio::test]
async fn a_listening_socket_of_a_live_process_passes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("s.sock");
    let _listener = tokio::net::UnixListener::bind(&path).expect("bind");
    let check = socket_check(std::process::id(), &path).await;
    assert_eq!(check.status, Status::Pass);
}

#[tokio::test]
async fn a_dead_pid_is_a_stale_socket() {
    let dir = tempfile::tempdir().expect("temp dir");
    let check = socket_check(u32::MAX - 1, &dir.path().join("s.sock")).await;
    assert_eq!(check.status, Status::Warn);
    assert!(check.title.contains("stale"));
}

#[tokio::test]
async fn a_live_process_with_no_listener_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    let check = socket_check(std::process::id(), &dir.path().join("none.sock")).await;
    assert_eq!(check.status, Status::Fail);
}
