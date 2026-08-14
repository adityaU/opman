use super::*;

#[test]
fn bare_host_gets_https() {
    assert_eq!(
        normalize_url("example.com/path").ok().as_deref(),
        Some("https://example.com/path")
    );
}

#[test]
fn explicit_scheme_is_preserved() {
    assert_eq!(
        normalize_url("http://localhost:3000").ok().as_deref(),
        Some("http://localhost:3000")
    );
}

#[test]
fn about_blank_is_allowed() {
    assert_eq!(
        normalize_url("about:blank").ok().as_deref(),
        Some("about:blank")
    );
}

#[test]
fn surrounding_whitespace_is_ignored() {
    assert_eq!(
        normalize_url("  example.com  ").ok().as_deref(),
        Some("https://example.com")
    );
}

#[test]
fn dangerous_schemes_are_refused() {
    for input in ["javascript:alert(1)", "data:text/html,<b>x", "chrome://gpu"] {
        assert!(
            normalize_url(input).is_err(),
            "{input} should not be navigable"
        );
    }
}

#[test]
fn empty_input_is_refused() {
    assert!(normalize_url("   ").is_err());
}
