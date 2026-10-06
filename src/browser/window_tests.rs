use super::*;

fn viewport() -> Viewport {
    Viewport::new(1280, 800, None)
}

#[test]
fn a_default_window_is_not_positioned() {
    let params = create_target_params(viewport(), Placement::Default);
    assert_eq!(params["newWindow"], json!(true));
    assert_eq!(params["width"], json!(1280));
    assert!(params.get("left").is_none());
    assert!(params.get("background").is_none());
}

#[test]
fn an_offscreen_window_is_created_off_screen_and_in_the_background() {
    let params = create_target_params(viewport(), Placement::OffScreen);
    assert_eq!(params["left"], json!(OFFSCREEN_ORIGIN));
    assert_eq!(params["top"], json!(OFFSCREEN_ORIGIN));
    assert_eq!(params["background"], json!(true));
    assert_eq!(params["height"], json!(800));
}

#[test]
fn parking_moves_a_normal_window_to_the_origin() {
    let params = park_params(7, viewport());
    assert_eq!(params["windowId"], json!(7));
    assert_eq!(params["bounds"]["left"], json!(OFFSCREEN_ORIGIN));
    assert_eq!(params["bounds"]["top"], json!(OFFSCREEN_ORIGIN));
    assert_eq!(params["bounds"]["windowState"], json!("normal"));
    assert_eq!(params["bounds"]["width"], json!(1280));
}
