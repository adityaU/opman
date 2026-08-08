//! Metadata probe order and resource canonicalisation.

use super::*;

#[test]
fn a_resource_uri_is_canonicalised() {
    assert_eq!(
        canonical_resource("https://mcp.example.com/mcp/")
            .expect("ok")
            .as_str(),
        "https://mcp.example.com/mcp"
    );
    assert_eq!(
        canonical_resource("https://mcp.example.com/mcp#frag")
            .expect("ok")
            .as_str(),
        "https://mcp.example.com/mcp"
    );
    assert_eq!(
        canonical_resource("https://mcp.example.com/mcp?a=b")
            .expect("ok")
            .as_str(),
        "https://mcp.example.com/mcp"
    );
}

#[test]
fn protected_resource_metadata_is_probed_path_first() {
    let resource = canonical_resource("https://x.example/mcp").expect("ok");
    let urls: Vec<String> = prm_urls(&resource).iter().map(Url::to_string).collect();
    assert_eq!(
        urls,
        [
            "https://x.example/.well-known/oauth-protected-resource/mcp",
            "https://x.example/.well-known/oauth-protected-resource",
        ]
    );
}

/// A client MUST support both discovery families, so all four forms must be tried.
#[test]
fn both_metadata_families_are_probed() {
    let issuer = Url::parse("https://as.example/tenant").expect("ok");
    let urls: Vec<String> = as_urls(&issuer).iter().map(Url::to_string).collect();
    assert_eq!(
        urls,
        [
            "https://as.example/.well-known/oauth-authorization-server/tenant",
            "https://as.example/.well-known/oauth-authorization-server",
            "https://as.example/.well-known/openid-configuration/tenant",
            "https://as.example/.well-known/openid-configuration",
        ]
    );
}
