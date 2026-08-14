use super::*;

/// Build a profile directory that looks the way Chromium leaves one.
fn claimed(pid: u32, port: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::os::unix::fs::symlink(format!("somehost-{pid}"), dir.path().join("SingletonLock"))
        .expect("lock");
    std::fs::write(dir.path().join("SingletonCookie"), "").expect("cookie");
    if let Some(port) = port {
        std::fs::write(dir.path().join("DevToolsActivePort"), port).expect("port file");
    }
    dir
}

#[test]
fn an_unclaimed_directory_is_free() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(owner(dir.path()), Owner::Free);
}

#[test]
fn a_claim_from_a_dead_process_is_stale() {
    // Pid 0 is never a process, so the /proc lookup fails the way a departed one does.
    let dir = claimed(0, Some("41234\n/devtools/browser/abc\n"));
    assert_eq!(owner(dir.path()), Owner::Stale);
}

#[test]
fn a_stale_claim_is_removable() {
    let dir = claimed(0, None);
    release(dir.path());
    assert!(!dir.path().join("SingletonLock").exists());
    assert!(!dir.path().join("SingletonCookie").exists());
    assert_eq!(owner(dir.path()), Owner::Free);
}

#[test]
fn a_published_endpoint_becomes_a_websocket_url() {
    let dir = claimed(0, Some("41234\n/devtools/browser/abc-123\n"));
    assert_eq!(
        devtools_endpoint(dir.path()).as_deref(),
        Some("ws://127.0.0.1:41234/devtools/browser/abc-123")
    );
}

#[test]
fn a_port_file_without_a_target_path_is_no_endpoint() {
    let dir = claimed(0, Some("41234\n"));
    assert!(devtools_endpoint(dir.path()).is_none());
}

/// The pid-reuse guard: a live pid only counts when it is still running *this* profile.
#[cfg(target_os = "linux")]
#[test]
fn a_live_pid_counts_only_while_its_command_line_names_the_profile() {
    let exe = std::env::current_exe().expect("current exe");
    assert!(holder_is_running(std::process::id(), &exe, true));
    assert!(!holder_is_running(
        std::process::id(),
        Path::new("/definitely/not/in/argv"),
        true
    ));
}

#[test]
fn a_hostname_with_dashes_still_yields_the_pid() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::os::unix::fs::symlink("build-box-01-4242", dir.path().join("SingletonLock"))
        .expect("lock");
    assert_eq!(lock_holder(dir.path()), Some(4242));
}
