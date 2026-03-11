//! Tests for the tunnel module.

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::web::tunnel::helpers::{extract_auth_url, extract_tunnel_url};
    use crate::web::tunnel::local_managed_setup::generate_config;
    use crate::web::tunnel::types::{tunnel_data_dir, TunnelOptions};

    #[test]
    fn test_extract_url_from_table() {
        let line = "|  https://foo-bar-baz.trycloudflare.com  |";
        assert_eq!(
            extract_tunnel_url(line),
            Some("https://foo-bar-baz.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_extract_url_from_log() {
        let line = "2025-03-05T10:00:00Z INF +---https://abc-123.trycloudflare.com registered";
        assert_eq!(
            extract_tunnel_url(line),
            Some("https://abc-123.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn test_no_url() {
        assert_eq!(extract_tunnel_url("some random log line"), None);
        assert_eq!(
            extract_tunnel_url("https://example.com is not a tunnel"),
            None
        );
    }

    #[test]
    fn test_tunnel_data_dir() {
        let dir = tunnel_data_dir().unwrap();
        assert!(dir.ends_with("opman/tunnel"));
    }

    #[test]
    fn test_generate_config() {
        let tmp = std::env::temp_dir().join("opman_test_config.json");
        let opts = TunnelOptions::default();
        generate_config(
            &tmp,
            "test-uuid-1234",
            Path::new("/tmp/tunnel.json"),
            "test.example.com",
            8080,
            &opts,
        )
        .unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&tmp).unwrap()).unwrap();
        assert_eq!(content["tunnel"], "test-uuid-1234");
        assert_eq!(content["ingress"][0]["hostname"], "test.example.com");
        assert_eq!(content["ingress"][0]["service"], "http://localhost:8080");
        assert_eq!(content["ingress"][1]["service"], "http_status:404");

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_extract_auth_url() {
        // Typical cloudflared login output
        let line = "Please open the following URL and log in with your Cloudflare account: https://dash.cloudflare.com/argotunnel?aud=abc&callback=https%3A%2F%2Flogin";
        assert_eq!(
            extract_auth_url(line),
            Some(
                "https://dash.cloudflare.com/argotunnel?aud=abc&callback=https%3A%2F%2Flogin"
                    .to_string()
            )
        );

        // No auth URL
        assert_eq!(extract_auth_url("some random log line"), None);
        assert_eq!(
            extract_auth_url("https://example.com is not cloudflare"),
            None
        );
    }
}
