use super::*;

#[test]
fn no_headers_means_framable() {
    assert_eq!(RenderMode::from_headers(None, None), RenderMode::Iframe);
}

#[test]
fn x_frame_options_refusals_force_screencast() {
    for value in ["DENY", "deny", "SAMEORIGIN", "sameorigin"] {
        assert_eq!(
            RenderMode::from_headers(Some(value), None),
            RenderMode::Screencast,
            "{value} should refuse framing"
        );
    }
}

#[test]
fn allow_from_is_not_a_blanket_refusal() {
    // ALLOW-FROM names a permitted origin rather than refusing outright, and is ignored
    // by every current browser — trying the iframe is the right call.
    assert_eq!(
        RenderMode::from_headers(Some("ALLOW-FROM https://example.com"), None),
        RenderMode::Iframe
    );
}

#[test]
fn csp_frame_ancestors_refusal_is_detected_among_other_directives() {
    let csp = "default-src 'self'; frame-ancestors 'none'; img-src *";
    assert_eq!(
        RenderMode::from_headers(None, Some(csp)),
        RenderMode::Screencast
    );
}

#[test]
fn csp_wildcard_frame_ancestors_permits_framing() {
    assert_eq!(
        RenderMode::from_headers(None, Some("frame-ancestors *")),
        RenderMode::Iframe
    );
}

#[test]
fn csp_without_frame_ancestors_permits_framing() {
    assert_eq!(
        RenderMode::from_headers(None, Some("default-src 'self'; script-src 'self'")),
        RenderMode::Iframe
    );
}

#[test]
fn snapshot_defaults_are_the_token_budget() {
    let options = SnapshotOptions::default();
    assert_eq!(options.max_nodes, 400);
    assert_eq!(options.max_chars, 12_000);
    assert!(!options.viewport_only, "a page read defaults to the page");
}

#[test]
fn a_pane_is_captured_at_the_ratio_its_display_asked_for() {
    let viewport = Viewport::new(900, 600, Some(2.0));
    assert_eq!((viewport.width(), viewport.height()), (900, 600));
    assert_eq!(viewport.scale(), 2.0);
}

#[test]
fn a_client_that_sends_no_ratio_gets_the_old_one_to_one_capture() {
    assert_eq!(Viewport::new(900, 600, None).scale(), 1.0);
    // Nonsense from the wire must not reach Chromium as a device scale factor.
    assert_eq!(Viewport::new(900, 600, Some(f64::NAN)).scale(), 1.0);
    assert_eq!(Viewport::new(900, 600, Some(-4.0)).scale(), 1.0);
    assert_eq!(Viewport::new(900, 600, Some(0.5)).scale(), 1.0);
}

#[test]
fn a_wide_pane_on_a_retina_display_is_not_allowed_an_unbounded_frame() {
    // 2000 CSS px at 2x would be a 4000px JPEG per frame, several times a second.
    let viewport = Viewport::new(2000, 1200, Some(2.0));
    assert!(viewport.scale() < 2.0, "scale was {}", viewport.scale());
    assert!(f64::from(viewport.width()) * viewport.scale() <= 2560.0);
    // Sharpness is given up before size: the pane still gets its full width.
    assert_eq!(viewport.width(), 2000);
}

#[test]
fn an_absurd_pane_size_is_clamped_to_something_chromium_accepts() {
    let tiny = Viewport::new(0, 0, Some(2.0));
    assert!(tiny.width() >= 200 && tiny.height() >= 200);
    let huge = Viewport::new(99_999, 99_999, None);
    assert_eq!((huge.width(), huge.height()), (3840, 2160));
}

#[test]
fn a_one_to_one_device_caps_a_retina_pane() {
    // Device mode does not force a scale: a 1x monitor's surface has 1x to give.
    let viewport = Viewport::new(900, 600, Some(2.0)).within_surface(1.0);
    assert_eq!(viewport.scale(), 1.0);
    assert_eq!(viewport.capture_width(), 900);
}

#[test]
fn a_surface_with_room_leaves_the_scale_alone() {
    let viewport = Viewport::new(900, 600, Some(1.5)).within_surface(2.0);
    assert_eq!(viewport.scale(), 1.5);
}

#[test]
fn a_nonsense_surface_ratio_falls_back_to_one() {
    let viewport = Viewport::new(900, 600, Some(2.0)).within_surface(f64::NAN);
    assert_eq!(viewport.scale(), 1.0);
}
