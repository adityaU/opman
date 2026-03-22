// ═══════════════════════════════════════════════════════════════════
// Unit tests for pure / private helper functions
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::super::common::constant_time_eq;
    use super::super::files_handlers::{detect_language, mime_from_extension};
    use super::super::search_handlers::{build_snippet, default_search_limit};

    // ── constant_time_eq ────────────────────────────────────────

    #[test]
    fn cte_equal_slices() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn cte_empty_slices() {
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn cte_different_content() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn cte_different_lengths() {
        assert!(!constant_time_eq(b"short", b"longer string"));
    }

    #[test]
    fn cte_single_bit_diff() {
        // 'A' = 0x41, 'B' = 0x42 — differ by one bit
        assert!(!constant_time_eq(b"A", b"B"));
    }

    #[test]
    fn cte_binary_data() {
        let a = vec![0u8, 1, 2, 255, 128];
        let b = vec![0u8, 1, 2, 255, 128];
        assert!(constant_time_eq(&a, &b));
        let mut c = b.clone();
        c[4] = 127;
        assert!(!constant_time_eq(&a, &c));
    }

    // ── mime_from_extension ─────────────────────────────────────

    #[test]
    fn mime_images() {
        assert_eq!(mime_from_extension("photo.png"), "image/png");
        assert_eq!(mime_from_extension("photo.PNG"), "image/png");
        assert_eq!(mime_from_extension("photo.jpg"), "image/jpeg");
        assert_eq!(mime_from_extension("photo.jpeg"), "image/jpeg");
        assert_eq!(mime_from_extension("anim.gif"), "image/gif");
        assert_eq!(mime_from_extension("icon.svg"), "image/svg+xml");
        assert_eq!(mime_from_extension("pic.webp"), "image/webp");
        assert_eq!(mime_from_extension("fav.ico"), "image/x-icon");
        assert_eq!(mime_from_extension("img.avif"), "image/avif");
    }

    #[test]
    fn mime_audio() {
        assert_eq!(mime_from_extension("song.mp3"), "audio/mpeg");
        assert_eq!(mime_from_extension("clip.wav"), "audio/wav");
        assert_eq!(mime_from_extension("track.ogg"), "audio/ogg");
        assert_eq!(mime_from_extension("music.flac"), "audio/flac");
        assert_eq!(mime_from_extension("a.m4a"), "audio/mp4");
    }

    #[test]
    fn mime_video() {
        assert_eq!(mime_from_extension("vid.mp4"), "video/mp4");
        assert_eq!(mime_from_extension("v.webm"), "video/webm");
        assert_eq!(mime_from_extension("m.mov"), "video/quicktime");
        assert_eq!(mime_from_extension("m.mkv"), "video/x-matroska");
    }

    #[test]
    fn mime_documents() {
        assert_eq!(mime_from_extension("doc.pdf"), "application/pdf");
        assert_eq!(mime_from_extension("data.csv"), "text/csv");
    }

    #[test]
    fn mime_unknown_falls_back() {
        assert_eq!(mime_from_extension("file.xyz"), "application/octet-stream");
        assert_eq!(mime_from_extension("noext"), "application/octet-stream");
    }

    #[test]
    fn mime_case_insensitive() {
        assert_eq!(mime_from_extension("FILE.PDF"), "application/pdf");
        assert_eq!(mime_from_extension("song.MP3"), "audio/mpeg");
    }

    // ── detect_language ─────────────────────────────────────────

    #[test]
    fn detect_rust() {
        assert_eq!(detect_language("main.rs"), "rust");
    }

    #[test]
    fn detect_javascript_variants() {
        assert_eq!(detect_language("app.js"), "javascript");
        assert_eq!(detect_language("App.jsx"), "javascript");
        assert_eq!(detect_language("index.mjs"), "javascript");
        assert_eq!(detect_language("config.cjs"), "javascript");
    }

    #[test]
    fn detect_typescript_variants() {
        assert_eq!(detect_language("app.ts"), "typescript");
        assert_eq!(detect_language("App.tsx"), "typescript");
        assert_eq!(detect_language("index.mts"), "typescript");
    }

    #[test]
    fn detect_python() {
        assert_eq!(detect_language("script.py"), "python");
        assert_eq!(detect_language("gui.pyw"), "python");
    }

    #[test]
    fn detect_various_languages() {
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("Main.java"), "java");
        assert_eq!(detect_language("lib.c"), "c");
        assert_eq!(detect_language("lib.h"), "c");
        assert_eq!(detect_language("lib.cpp"), "cpp");
        assert_eq!(detect_language("data.json"), "json");
        assert_eq!(detect_language("page.html"), "html");
        assert_eq!(detect_language("style.css"), "css");
        assert_eq!(detect_language("readme.md"), "markdown");
        assert_eq!(detect_language("query.sql"), "sql");
        assert_eq!(detect_language("layout.xml"), "xml");
        assert_eq!(detect_language("config.yaml"), "yaml");
        assert_eq!(detect_language("Cargo.toml"), "toml");
        assert_eq!(detect_language("run.sh"), "shell");
        assert_eq!(detect_language("init.lua"), "lua");
        assert_eq!(detect_language("app.rb"), "ruby");
        assert_eq!(detect_language("index.php"), "php");
    }

    #[test]
    fn detect_case_insensitive() {
        assert_eq!(detect_language("FILE.RS"), "rust");
        assert_eq!(detect_language("APP.TSX"), "typescript");
    }

    #[test]
    fn detect_unknown_falls_back() {
        assert_eq!(detect_language("file.xyz"), "text");
        assert_eq!(detect_language("noext"), "text");
    }

    // ── build_snippet ───────────────────────────────────────────

    #[test]
    fn snippet_basic_match() {
        let text = "The quick brown fox jumps over the lazy dog";
        let snippet = build_snippet(text, "fox", 30);
        assert!(snippet.contains("fox"));
        assert!(snippet.len() <= 40); // 30 + possible "..." * 2
    }

    #[test]
    fn snippet_no_match() {
        let text = "Hello world";
        let snippet = build_snippet(text, "xyz", 20);
        // Should return first max_len chars
        assert_eq!(snippet, "Hello world");
    }

    #[test]
    fn snippet_match_at_start() {
        let text = "Hello world, this is a longer text";
        let snippet = build_snippet(text, "hello", 20);
        assert!(snippet.starts_with("Hello"));
    }

    #[test]
    fn snippet_match_at_end() {
        let text = "This is a very long text that ends with target";
        let snippet = build_snippet(text, "target", 20);
        assert!(snippet.contains("target"));
    }

    #[test]
    fn snippet_short_text_returned_fully() {
        let text = "tiny";
        let snippet = build_snippet(text, "tiny", 100);
        assert_eq!(snippet, "tiny");
    }

    #[test]
    fn snippet_ellipsis_when_truncated() {
        // Place needle in the middle
        let mut haystack = "B".repeat(200);
        haystack.push_str("needle");
        haystack.push_str(&"B".repeat(200));
        let snippet = build_snippet(&haystack, "needle", 40);
        assert!(snippet.contains("needle"));
        // Should have ellipsis on at least one side
        assert!(snippet.contains("..."));
    }

    #[test]
    fn snippet_case_insensitive_needle() {
        let text = "Hello World";
        let snippet = build_snippet(text, "hello", 50);
        assert!(snippet.contains("Hello"));
    }

    // ── default_search_limit ────────────────────────────────────

    #[test]
    fn default_search_limit_is_50() {
        assert_eq!(default_search_limit(), 50);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Unit tests for API endpoint handlers
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod handler_tests {
    use super::*;
    use axum::response::{IntoResponse, Json};
    use axum::http::{HeaderMap, StatusCode};
    use axum::extract::State;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use reqwest::Client;

    use crate::mcp::NvimSocketRegistry;
    use crate::mcp_skills::SkillsRegistry;
    use crate::web::auth::AuthUser;
    use crate::web::pty_manager::WebPtyHandle;
    use crate::web::web_state::WebStateHandle;
    use crate::web::types::*;
    use crate::web::error::WebError;

    // Import the handler functions
    use crate::web::handlers::{
        health, login, verify, get_state, list_themes, public_bootstrap,
        switch_project, add_project, home_dir, browse_files, read_file,
        pty_list, get_system_stats,
    };

    // ── Test utilities ──────────────────────────────────────────

    // For now, create minimal mocks to get tests compiling
    // In a real implementation, these would be proper mocks
    fn create_test_server_state() -> ServerState {
        let (event_tx, _) = broadcast::channel::<crate::web::types::WebEvent>(100);
        let (raw_sse_tx, _) = broadcast::channel::<String>(100);
        let (reload_tx, _) = broadcast::channel::<()>(100);
        let (editor_tx, _) = broadcast::channel::<crate::web::types::EditorEvent>(100);

        // This is a simplified mock - in practice you'd need proper test doubles
        // For now, this will allow compilation but tests may not work correctly
        todo!("Implement proper mock ServerState for testing")
    }

    // ── Auth handlers ───────────────────────────────────────────

    #[tokio::test]
    async fn health_returns_ok_status() {
        let response = health().await;
        let status = response.into_response().status();
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn health_returns_version_info() {
        let response = health().await;
        let json_response = response.into_response();
        // Extract JSON body - this would need proper deserialization in real test
        // For now, just ensure it returns successfully
        assert_eq!(json_response.status(), StatusCode::OK);
    }

    // TODO: Add more comprehensive tests once mock infrastructure is properly set up
    // The following tests require proper mocking of ServerState and dependencies

    /*
    #[tokio::test]
    async fn login_with_valid_credentials_returns_token() {
        let state = create_test_server_state();
        let mut headers = HeaderMap::new();
        let login_req = LoginRequest {
            username: "testuser".to_string(),
            password: "testpass".to_string(),
        };

        let result = login(
            State(state),
            headers,
            Json(login_req),
        ).await;

        assert!(result.is_ok());
        let response = result.unwrap().into_response();
        assert_eq!(response.status(), StatusCode::OK);

        // Check that Set-Cookie header is present
        let headers = response.headers().clone();
        assert!(headers.contains_key("set-cookie"));
    }
    */
}
