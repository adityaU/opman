use super::*;

#[test]
fn the_path_follows_the_marker_line_despite_banners() {
    let output = format!("Welcome!\n{MARKER}\n\n/opt/homebrew/bin:/usr/bin\nbye\n");
    assert_eq!(
        marked_line(output.as_bytes()).as_deref(),
        Some("/opt/homebrew/bin:/usr/bin")
    );
}

#[test]
fn no_marker_means_no_path() {
    assert_eq!(marked_line("just a banner\n".as_bytes()), None);
}

#[test]
fn merge_keeps_shell_order_and_drops_duplicates() {
    let extras = vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/x/bin")];
    let inherited = OsString::from("/usr/bin:/x/bin:/bin");
    let merged = merge(Some("/a:/opt/homebrew/bin:/usr/bin"), &extras, &inherited);
    let expected: Vec<PathBuf> = ["/a", "/opt/homebrew/bin", "/usr/bin", "/x/bin", "/bin"]
        .iter()
        .map(PathBuf::from)
        .collect();
    assert_eq!(merged, expected);
}

#[test]
fn a_failed_shell_still_gets_known_dirs_before_the_bare_path() {
    let extras = vec![PathBuf::from("/opt/homebrew/bin")];
    let merged = merge(None, &extras, &OsString::from("/usr/bin:/bin"));
    assert_eq!(merged.first(), Some(&PathBuf::from("/opt/homebrew/bin")));
}

#[test]
fn nvm_prefers_the_default_alias_then_the_newest() {
    let nvm = tempfile::tempdir().expect("tempdir");
    for version in ["v18.19.0", "v20.11.1", "v9.0.0"] {
        std::fs::create_dir_all(nvm.path().join("versions/node").join(version).join("bin"))
            .expect("version dir");
    }
    let newest = nvm_bin(nvm.path()).expect("an installed node");
    assert!(newest.ends_with("v20.11.1/bin"), "{}", newest.display());

    std::fs::create_dir_all(nvm.path().join("alias")).expect("alias dir");
    std::fs::write(nvm.path().join("alias/default"), "18\n").expect("alias");
    let aliased = nvm_bin(nvm.path()).expect("the aliased node");
    assert!(aliased.ends_with("v18.19.0/bin"), "{}", aliased.display());
}

#[test]
fn programs_are_found_on_the_merged_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("npx"), "").expect("fake npx");
    let path = std::env::join_paths([dir.path()]).expect("path");
    assert_eq!(find_in(&path, "npx"), Some(dir.path().join("npx")));
    assert_eq!(find_in(&path, "missing"), None);
}

#[cfg(unix)]
#[test]
fn a_real_login_shell_reports_a_path() {
    // bash is everywhere CI runs these tests; the marker parsing is what is under test.
    let path = read_login_path("/bin/bash".as_ref()).expect("bash prints its PATH");
    assert!(path.contains("/usr/bin"), "{path}");
}

#[cfg(unix)]
#[test]
fn a_daemon_holding_stdout_does_not_hang_the_read() {
    use std::os::unix::fs::PermissionsExt;
    // A profile that prints the PATH, then leaves a background job owning stdout — the
    // way an ssh-agent or a tmux auto-start does.
    let dir = tempfile::tempdir().expect("tempdir");
    let shell = dir.path().join("fake-shell");
    std::fs::write(
        &shell,
        format!("#!/bin/sh\necho banner\necho {MARKER}\necho /fake/bin:/usr/bin\nsleep 30 &\n"),
    )
    .expect("script");
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let started = std::time::Instant::now();
    let path = read_login_path(shell.as_os_str()).expect("the PATH line");
    assert_eq!(path, "/fake/bin:/usr/bin");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}
