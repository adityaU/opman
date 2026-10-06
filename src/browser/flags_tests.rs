use super::*;

fn flags(mode: BrowserMode) -> Vec<&'static str> {
    launch_flags(mode).collect()
}

#[test]
fn the_launch_scale_matches_the_ratio_a_pane_may_ask_for() {
    // A pane allowed to ask for more than the browser renders would get a frame whose
    // size no longer matches the coordinate space it is clicking in.
    let expected = format!(
        "--force-device-scale-factor={}",
        super::super::types::MAX_RATIO
    );
    assert_eq!(DEVICE_SCALE_FLAG, expected);
}

#[test]
fn only_server_mode_forces_a_scale_factor() {
    assert!(flags(BrowserMode::Server).contains(&DEVICE_SCALE_FLAG));
    assert!(!flags(BrowserMode::Device)
        .iter()
        .any(|flag| flag.starts_with("--force-device-scale-factor")));
}

#[test]
fn both_modes_keep_hidden_windows_painting() {
    for mode in [BrowserMode::Server, BrowserMode::Device] {
        let flags = flags(mode);
        assert!(flags.contains(&"--disable-backgrounding-occluded-windows"));
        assert!(flags.contains(&"--disable-renderer-backgrounding"));
        assert!(flags.contains(&"--remote-debugging-port=0"));
    }
}

#[test]
fn device_mode_disables_throttling_and_opens_off_screen() {
    let flags = flags(BrowserMode::Device);
    assert!(flags.contains(&"--disable-background-timer-throttling"));
    assert!(flags.contains(&"--no-startup-window"));
    assert!(flags.contains(&OFFSCREEN_POSITION_FLAG));
    assert!(!flags.contains(&"--window-position=0,0"));
}

#[test]
fn device_mode_never_asks_for_headless() {
    assert!(!flags(BrowserMode::Device)
        .iter()
        .any(|flag| flag.starts_with("--headless")));
}

#[test]
fn the_position_flag_matches_the_parking_origin() {
    let origin = super::super::mode::OFFSCREEN_ORIGIN;
    assert_eq!(
        OFFSCREEN_POSITION_FLAG,
        format!("--window-position={origin},{origin}")
    );
}

#[test]
fn server_mode_keeps_its_container_flags() {
    let flags = flags(BrowserMode::Server);
    assert!(flags.contains(&"--disable-dev-shm-usage"));
    assert!(flags.contains(&"--disable-background-networking"));
}
