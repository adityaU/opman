use super::*;

#[test]
fn an_x_display_wins_even_under_wayland() {
    // XWayland lets the browser position its windows; native Wayland does not.
    assert_eq!(classify(Some(":0".into()), true), Session::X(":0".into()));
    assert_eq!(classify(Some(":1".into()), false), Session::X(":1".into()));
}

#[test]
fn wayland_alone_is_used_natively() {
    assert_eq!(classify(None, true), Session::Wayland);
}

#[test]
fn no_display_at_all_is_reported_as_such() {
    assert_eq!(classify(None, false), Session::None);
}

#[test]
fn the_native_display_never_asks_for_headless_or_x11() {
    let flags = Display::Native.chrome_flags();
    assert!(!flags.contains(&"--headless=new"));
    assert!(!flags.contains(&"--ozone-platform=x11"));
}

#[test]
fn the_native_display_keeps_the_desktop_environment() {
    let mut command = tokio::process::Command::new("true");
    command.env("DISPLAY", ":42");
    Display::Native.apply_env(&mut command);
    let display = command
        .as_std()
        .get_envs()
        .find(|(key, _)| *key == "DISPLAY")
        .and_then(|(_, value)| value);
    assert_eq!(display, Some(std::ffi::OsStr::new(":42")));
}

#[test]
fn the_headless_fallback_drops_a_stale_display() {
    let mut command = tokio::process::Command::new("true");
    Display::Headless.apply_env(&mut command);
    let removed = command
        .as_std()
        .get_envs()
        .any(|(key, value)| key == "DISPLAY" && value.is_none());
    assert!(removed);
}
