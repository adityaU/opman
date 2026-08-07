//! RFC 9207 response validation, in the order the spec demands.

use super::*;

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn a_matching_response_yields_the_code() {
    let p = params(&[("state", "s"), ("iss", "https://as"), ("code", "abc")]);
    assert_eq!(
        validate_response(&p, "s", "https://as", true).expect("ok"),
        "abc"
    );
}

#[test]
fn a_state_mismatch_is_rejected_before_anything_else_is_read() {
    // Including before `error`: an attacker-supplied response must not be able to make
    // opman display its message.
    let p = params(&[("state", "wrong"), ("error", "access_denied")]);
    assert!(matches!(
        validate_response(&p, "s", "https://as", false),
        Err(OAuthError::StateMismatch)
    ));
}

#[test]
fn issuer_comparison_is_byte_exact() {
    // A trailing slash is a different issuer; no normalisation is permitted.
    let p = params(&[("state", "s"), ("iss", "https://as/"), ("code", "abc")]);
    assert!(matches!(
        validate_response(&p, "s", "https://as", true),
        Err(OAuthError::IssuerMismatch)
    ));
}

#[test]
fn an_absent_iss_is_rejected_only_when_the_server_advertised_it() {
    let p = params(&[("state", "s"), ("code", "abc")]);
    assert!(matches!(
        validate_response(&p, "s", "https://as", true),
        Err(OAuthError::IssuerMismatch)
    ));
    assert!(validate_response(&p, "s", "https://as", false).is_ok());
}

/// On an issuer mismatch the error fields must not be acted on or displayed — so the
/// variant deliberately carries none of them.
#[test]
fn an_issuer_mismatch_discards_the_servers_error_text() {
    let p = params(&[
        ("state", "s"),
        ("iss", "https://evil"),
        ("error", "access_denied"),
        ("error_description", "do not show me"),
    ]);
    let error = validate_response(&p, "s", "https://as", true).expect_err("must fail");
    assert!(matches!(error, OAuthError::IssuerMismatch));
    assert!(!error.to_string().contains("do not show me"));
}

#[test]
fn a_denial_is_reported_once_state_and_issuer_check_out() {
    let p = params(&[("state", "s"), ("error", "access_denied")]);
    assert!(matches!(
        validate_response(&p, "s", "https://as", false),
        Err(OAuthError::Denied(_))
    ));
}

#[test]
fn query_parsing_percent_decodes() {
    let parsed = parse_query("code=a%20b&state=s");
    assert_eq!(parsed.get("code").map(String::as_str), Some("a b"));
    assert_eq!(parsed.get("state").map(String::as_str), Some("s"));
}
