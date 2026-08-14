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
