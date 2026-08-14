use super::*;

#[test]
fn which_finds_nothing_for_an_impossible_name() {
    assert!(which("opman-definitely-not-a-real-binary").is_none());
}

#[test]
fn a_path_under_snap_is_a_snap_without_reading_it() {
    assert!(is_snap(Path::new("/snap/bin/chromium")));
}

#[test]
fn a_shim_that_execs_a_snap_is_a_snap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shim = dir.path().join("chromium-browser");
    std::fs::write(&shim, "#!/bin/sh\nexec /snap/bin/chromium \"$@\"\n").expect("write shim");
    assert!(is_snap(&shim));
}

#[test]
fn an_ordinary_binary_is_not_a_snap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exe = dir.path().join("chrome");
    std::fs::write(&exe, [0x7f, b'E', b'L', b'F', 0, 0, 0, 0]).expect("write exe");
    assert!(!is_snap(&exe));
}
