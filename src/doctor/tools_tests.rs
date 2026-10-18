use super::*;
use crate::doctor::report::Status;
use std::os::unix::fs::PermissionsExt;

fn bin_dir(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    for name in names {
        let file = dir.path().join(name);
        std::fs::write(&file, "#!/bin/sh\n").expect("write");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    dir
}

fn status_of(checks: &[Check], name: &str) -> Status {
    checks
        .iter()
        .find(|c| c.title.starts_with(name))
        .map(|c| c.status)
        .expect("a check for the tool")
}

#[test]
fn finds_an_executable_and_ignores_a_plain_file() {
    let dir = bin_dir(&["git"]);
    std::fs::write(dir.path().join("data"), "x").expect("write");
    let path = dir.path().as_os_str();
    assert!(find_on_path("git", Some(path)).is_some());
    assert!(find_on_path("data", Some(path)).is_none());
    assert!(find_on_path("git", None).is_none());
}

#[test]
fn a_missing_optional_tool_is_a_warning() {
    let dir = bin_dir(&["git"]);
    let checks = checks(&[], Some(dir.path().as_os_str()));
    assert_eq!(status_of(&checks, "git"), Status::Pass);
    assert_eq!(status_of(&checks, "nvim"), Status::Warn);
}

#[test]
fn git_is_always_required() {
    let dir = bin_dir(&[]);
    let checks = checks(&[], Some(dir.path().as_os_str()));
    assert_eq!(status_of(&checks, "git"), Status::Fail);
}

#[test]
fn a_tool_an_enabled_agent_runs_is_required_with_its_base() {
    let dir = bin_dir(&["git"]);
    let checks = checks(&["npx"], Some(dir.path().as_os_str()));
    assert_eq!(status_of(&checks, "npx"), Status::Fail);
    assert_eq!(status_of(&checks, "node"), Status::Fail);
    assert_eq!(status_of(&checks, "uvx"), Status::Warn);
}

#[test]
fn command_name_drops_the_directory() {
    assert_eq!(command_name("/usr/local/bin/npx"), "npx");
    assert_eq!(command_name("uvx"), "uvx");
}
