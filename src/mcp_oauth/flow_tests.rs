//! The parts of the flow that are pure, which is where the spec's traps live.

use super::*;

#[test]
fn the_pkce_challenge_matches_the_rfc_vector() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

#[test]
fn a_generated_verifier_and_challenge_agree() {
    let pkce = Pkce::generate();
    assert_eq!(pkce.verifier.len(), 64);
    let digest = Sha256::digest(pkce.verifier.as_bytes());
    let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    assert_eq!(pkce.challenge, expected);
}

// ── RFC 9207 validation, in the order the spec demands ───────────────────────────────
