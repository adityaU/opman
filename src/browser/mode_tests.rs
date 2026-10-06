use super::*;

#[test]
fn the_flag_selects_device_mode() {
    assert_eq!(
        BrowserMode::from_device_flag(DeviceBrowser::from(true)),
        BrowserMode::Device
    );
    assert_eq!(
        BrowserMode::from_device_flag(DeviceBrowser::from(false)),
        BrowserMode::Server
    );
}

#[test]
fn server_is_the_default() {
    assert_eq!(BrowserMode::default(), BrowserMode::Server);
}

#[test]
fn each_mode_has_its_own_profile() {
    assert_eq!(BrowserMode::Server.profile_dir_name(), "browser-profile");
    assert_eq!(
        BrowserMode::Device.profile_dir_name(),
        "device-browser-profile"
    );
}

#[test]
fn only_device_windows_are_parked() {
    assert_eq!(BrowserMode::Server.placement(), Placement::Default);
    assert_eq!(BrowserMode::Device.placement(), Placement::OffScreen);
}

#[test]
fn the_offscreen_origin_fits_an_x11_coordinate() {
    assert!(i32::from(i16::MIN) < OFFSCREEN_ORIGIN && OFFSCREEN_ORIGIN < 0);
}

#[test]
fn the_mode_serializes_for_the_status_field() {
    assert_eq!(
        serde_json::to_value(BrowserMode::Device).ok(),
        Some(serde_json::json!("device"))
    );
}
