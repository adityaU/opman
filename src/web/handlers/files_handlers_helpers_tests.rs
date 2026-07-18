//! Generated coverage tests for `files_handlers.rs` — pure helpers
//! (mime/language detection, sync search, gitignore matching).

use super::*;

// ── mime_from_extension ────────────────────────────────────────────

#[test]
fn mime_images() {
    assert_eq!(mime_from_extension("a.png"), "image/png");
    assert_eq!(mime_from_extension("a.jpg"), "image/jpeg");
    assert_eq!(mime_from_extension("a.jpeg"), "image/jpeg");
    assert_eq!(mime_from_extension("a.gif"), "image/gif");
    assert_eq!(mime_from_extension("a.svg"), "image/svg+xml");
    assert_eq!(mime_from_extension("a.webp"), "image/webp");
    assert_eq!(mime_from_extension("a.ico"), "image/x-icon");
    assert_eq!(mime_from_extension("a.bmp"), "image/bmp");
    assert_eq!(mime_from_extension("a.avif"), "image/avif");
}

#[test]
fn mime_audio_video() {
    assert_eq!(mime_from_extension("a.mp3"), "audio/mpeg");
    assert_eq!(mime_from_extension("a.wav"), "audio/wav");
    assert_eq!(mime_from_extension("a.ogg"), "audio/ogg");
    assert_eq!(mime_from_extension("a.flac"), "audio/flac");
    assert_eq!(mime_from_extension("a.aac"), "audio/aac");
    assert_eq!(mime_from_extension("a.m4a"), "audio/mp4");
    assert_eq!(mime_from_extension("a.weba"), "audio/webm");
    assert_eq!(mime_from_extension("a.mp4"), "video/mp4");
    assert_eq!(mime_from_extension("a.webm"), "video/webm");
    assert_eq!(mime_from_extension("a.ogv"), "video/ogg");
    assert_eq!(mime_from_extension("a.mov"), "video/quicktime");
    assert_eq!(mime_from_extension("a.avi"), "video/x-msvideo");
    assert_eq!(mime_from_extension("a.mkv"), "video/x-matroska");
}

#[test]
fn mime_documents_and_fallback() {
    assert_eq!(mime_from_extension("a.pdf"), "application/pdf");
    assert_eq!(mime_from_extension("a.csv"), "text/csv");
    assert!(mime_from_extension("a.xlsx").contains("spreadsheetml"));
    assert!(mime_from_extension("a.ppt").contains("presentationml"));
    assert!(mime_from_extension("a.docx").contains("wordprocessingml"));
    // uppercase extension is lowercased
    assert_eq!(mime_from_extension("A.PNG"), "image/png");
    // no extension / unknown → octet-stream
    assert_eq!(mime_from_extension("noext"), "application/octet-stream");
    assert_eq!(mime_from_extension("a.zzz"), "application/octet-stream");
}

// ── detect_language ────────────────────────────────────────────────

#[test]
fn detect_language_common() {
    assert_eq!(detect_language("a.rs"), "rust");
    assert_eq!(detect_language("a.js"), "javascript");
    assert_eq!(detect_language("a.mjs"), "javascript");
    assert_eq!(detect_language("a.ts"), "typescript");
    assert_eq!(detect_language("a.tsx"), "typescript");
    assert_eq!(detect_language("a.py"), "python");
    assert_eq!(detect_language("a.go"), "go");
    assert_eq!(detect_language("a.java"), "java");
    assert_eq!(detect_language("a.c"), "c");
    assert_eq!(detect_language("a.h"), "c");
    assert_eq!(detect_language("a.cpp"), "cpp");
    assert_eq!(detect_language("a.hpp"), "cpp");
    assert_eq!(detect_language("a.json"), "json");
    assert_eq!(detect_language("a.html"), "html");
    assert_eq!(detect_language("a.css"), "css");
    assert_eq!(detect_language("a.scss"), "css");
}

#[test]
fn detect_language_more() {
    assert_eq!(detect_language("README.md"), "markdown");
    assert_eq!(detect_language("a.sql"), "sql");
    assert_eq!(detect_language("a.xml"), "xml");
    assert_eq!(detect_language("a.yaml"), "yaml");
    assert_eq!(detect_language("a.yml"), "yaml");
    assert_eq!(detect_language("a.toml"), "toml");
    assert_eq!(detect_language("a.sh"), "shell");
    assert_eq!(detect_language("a.fish"), "shell");
    assert_eq!(detect_language("a.lua"), "lua");
    assert_eq!(detect_language("a.rb"), "ruby");
    assert_eq!(detect_language("a.php"), "php");
    assert_eq!(detect_language("a.vue"), "vue");
    assert_eq!(detect_language("a.svelte"), "svelte");
    assert_eq!(detect_language("a.kt"), "kotlin");
    assert_eq!(detect_language("a.swift"), "swift");
    assert_eq!(detect_language("a.mmd"), "mermaid");
    assert_eq!(detect_language("a.ini"), "ini");
    assert_eq!(detect_language("a.proto"), "protobuf");
    assert_eq!(detect_language("a.graphql"), "graphql");
    assert_eq!(detect_language("a.diff"), "diff");
    assert_eq!(detect_language("a.dockerfile"), "dockerfile");
    assert_eq!(detect_language("a.makefile"), "makefile");
    // fallback
    assert_eq!(detect_language("a.unknownext"), "text");
    assert_eq!(detect_language("noext"), "text");
}

// ── load_gitignore ─────────────────────────────────────────────────

#[test]
fn load_gitignore_missing_returns_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(load_gitignore(tmp.path()).is_empty());
}

#[test]
fn load_gitignore_filters_comments_and_blanks() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join(".gitignore"),
        "# comment\n\n*.log\n  build/  \n",
    )
    .unwrap();
    let pats = load_gitignore(tmp.path());
    assert!(pats.contains(&"*.log".to_string()));
    assert!(pats.contains(&"build/".to_string()));
    assert!(!pats.iter().any(|p| p.starts_with('#')));
    assert_eq!(pats.len(), 2);
}

// ── is_gitignored ──────────────────────────────────────────────────

#[test]
fn is_gitignored_always_skip_dirs() {
    assert!(is_gitignored("node_modules", "node_modules", true, &[]));
    assert!(is_gitignored("target", "target", true, &[]));
    // a file named the same is NOT auto-skipped (only dirs)
    assert!(!is_gitignored("target", "target", false, &[]));
}

#[test]
fn is_gitignored_glob_extension() {
    let pats = vec!["*.pyc".to_string()];
    assert!(is_gitignored("a/b.pyc", "b.pyc", false, &pats));
    assert!(!is_gitignored("a/b.py", "b.py", false, &pats));
}

#[test]
fn is_gitignored_path_prefix() {
    let pats = vec!["dist/".to_string()];
    assert!(is_gitignored("dist/app.js", "app.js", false, &pats));
    let pats2 = vec!["build/output".to_string()];
    assert!(is_gitignored("build/output", "output", false, &pats2));
}

#[test]
fn is_gitignored_plain_name_and_segment() {
    let pats = vec!["secrets".to_string()];
    // exact name match
    assert!(is_gitignored("secrets", "secrets", false, &pats));
    // path segment match
    assert!(is_gitignored("a/secrets/b.txt", "b.txt", false, &pats));
    // no match
    assert!(!is_gitignored("a/other/b.txt", "b.txt", false, &pats));
}

// ── search_files_sync ──────────────────────────────────────────────

#[test]
fn search_files_sync_finds_matches() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("alpha.txt"), "").unwrap();
    std::fs::write(tmp.path().join("beta.txt"), "").unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub/alpha_nested.txt"), "").unwrap();

    let root = tmp.path().to_string_lossy().to_string();
    let results = search_files_sync(&root, "alpha", 10);
    assert!(results.iter().any(|e| e.name == "alpha.txt"));
    assert!(results.iter().any(|e| e.path.contains("alpha_nested")));
    // Both matches are found (directory-walk order is not guaranteed).
    assert!(results.iter().any(|e| e.path == "alpha.txt"));
}

#[test]
fn search_files_sync_no_matches() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("alpha.txt"), "").unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    assert!(search_files_sync(&root, "zzzznotfound", 10).is_empty());
}

#[test]
fn search_files_sync_multi_term() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "").unwrap();
    std::fs::write(tmp.path().join("readme.rs"), "").unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    // both "src" and "main" must appear in the path
    let results = search_files_sync(&root, "src main", 10);
    assert!(results.iter().any(|e| e.path.contains("main.rs")));
    assert!(!results.iter().any(|e| e.name == "readme.rs"));
}

#[test]
fn search_files_sync_skips_hidden_and_gitignored() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".gitignore"), "ignored.txt\n").unwrap();
    std::fs::write(tmp.path().join("ignored.txt"), "").unwrap();
    std::fs::write(tmp.path().join(".hidden.txt"), "").unwrap();
    std::fs::write(tmp.path().join("visible.txt"), "").unwrap();
    std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
    std::fs::write(tmp.path().join("node_modules/pkg.txt"), "").unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let results = search_files_sync(&root, "txt", 20);
    let names: Vec<_> = results.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"visible.txt".to_string()));
    assert!(!names.contains(&"ignored.txt".to_string()));
    assert!(!names.iter().any(|n| n.starts_with('.')));
    assert!(!names.contains(&"pkg.txt".to_string()));
}

#[test]
fn search_files_sync_whitespace_query_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "").unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    // all-whitespace query → no terms → empty
    assert!(search_files_sync(&root, "    ", 10).is_empty());
}

#[test]
fn search_files_sync_limit_and_heap_rebalance() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Create many matching files to force the heap-bounding rebalance branch.
    for i in 0..60 {
        std::fs::write(tmp.path().join(format!("match_{i:03}.txt")), "").unwrap();
    }
    let root = tmp.path().to_string_lossy().to_string();
    let results = search_files_sync(&root, "match", 2);
    assert_eq!(results.len(), 2);
}

#[test]
fn search_files_sync_bad_root_is_empty() {
    let results = search_files_sync("/nonexistent/dir/xyz", "q", 10);
    assert!(results.is_empty());
}
