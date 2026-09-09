use super::*;

static BROWSER_BIN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct BrowserBinGuard {
    previous: Option<std::ffi::OsString>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for BrowserBinGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(BROWSER_BIN_ENV, value),
            None => std::env::remove_var(BROWSER_BIN_ENV),
        }
    }
}

fn browser_bin_override(path: Option<&Path>) -> BrowserBinGuard {
    let lock = BROWSER_BIN_LOCK.lock().expect("browser env lock poisoned");
    let previous = std::env::var_os(BROWSER_BIN_ENV);
    match path {
        Some(path) => std::env::set_var(BROWSER_BIN_ENV, path),
        None => std::env::remove_var(BROWSER_BIN_ENV),
    }
    BrowserBinGuard {
        previous,
        _lock: lock,
    }
}

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

#[test]
fn install_guide_explains_the_browser_path_override() {
    let guide = install_guide();
    assert_eq!(guide.env_var, BROWSER_BIN_ENV);
    assert!(!guide.title.is_empty());
    assert!(!guide.summary.is_empty());
    assert!(!guide.docs_url.is_empty());
}

#[test]
fn explicit_missing_override_is_authoritative() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("missing-chromium");
    let _override = browser_bin_override(Some(&path));

    assert!(matches!(
        find(),
        Err(BrowserUnavailable::OverrideMissing(found)) if found == path
    ));
}

#[cfg(unix)]
#[test]
fn explicit_non_executable_override_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("not-executable-chromium");
    std::fs::write(&path, b"not a browser").expect("write browser placeholder");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("set non-executable permissions");
    let _override = browser_bin_override(Some(&path));

    assert!(matches!(
        find(),
        Err(BrowserUnavailable::OverrideNotExecutable(found)) if found == path
    ));
}

#[cfg(unix)]
#[test]
fn explicit_executable_override_is_selected() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("selected-chromium");
    std::fs::write(&path, b"#!/bin/sh\n").expect("write browser placeholder");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("set executable permissions");
    let _override = browser_bin_override(Some(&path));

    assert_eq!(find().ok(), Some(path));
}
