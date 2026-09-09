use super::*;

#[test]
fn only_the_disabled_variant_passes_sandbox_flags() {
    assert!(Sandbox::Enabled.flags().is_empty());
    assert!(Sandbox::Disabled.flags().contains(&"--no-sandbox"));
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
