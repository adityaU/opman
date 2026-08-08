//! End-to-end tests against a real language server.
//!
//! These spawn an actual `rust-analyzer` and are `#[ignore]`d so an ordinary
//! `cargo test` on a machine without it stays green. Run them deliberately:
//!
//! ```text
//! cargo test --  --ignored lsp::live
//! ```
//!
//! They are the only proof that the handshake, capability negotiation, document
//! sync and position conversion all line up against a server that did not read
//! our assumptions.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::pool::LspPool;

/// A tiny but real Cargo project, so rust-analyzer has something to index that
/// is not the whole opman workspace.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    dir
}

fn have_rust_analyzer() -> bool {
    super::detect::resolve_binary("rust-analyzer").is_some()
}

#[tokio::test]
#[ignore = "spawns a real rust-analyzer"]
async fn starts_rust_analyzer_and_reports_capabilities() {
    if !have_rust_analyzer() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let dir = fixture();
    let file = dir.path().join("src/main.rs");
    std::fs::write(
        &file,
        "fn main() {\n    let n: i32 = 1;\n    println!(\"{n}\");\n}\n",
    )
    .unwrap();

    let pool = Arc::new(LspPool::new());
    let resolved = pool
        .resolve(&file, dir.path())
        .await
        .expect("resolve should not error")
        .expect("rust-analyzer should be selected for a .rs file");

    // The root must be the crate, not the file's directory.
    assert!(
        resolved.root_is(dir.path()),
        "server must be rooted at the crate, not the file's directory"
    );

    let caps = resolved.server.ready().await.expect("handshake");
    assert!(caps.hover, "rust-analyzer advertises hover");
    assert!(caps.definition, "rust-analyzer advertises definition");

    pool.shutdown_all().await;
}

/// The headline behaviour: hover a symbol and get its type back, with no
/// Neovim anywhere.
#[tokio::test]
#[ignore = "spawns a real rust-analyzer"]
async fn hover_returns_type_information() {
    if !have_rust_analyzer() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let dir = fixture();
    let file = dir.path().join("src/main.rs");
    let source = "fn main() {\n    let answer: i32 = 42;\n    let _ = answer;\n}\n";
    std::fs::write(&file, source).unwrap();

    let pool = Arc::new(LspPool::new());
    // Column 9 on line 2 is inside `answer`.
    let value = wait_for_hover(&pool, &file, dir.path(), source, 2, 9).await;

    assert_eq!(value["available"], true, "LSP should be available: {value}");
    let hover = value["hover"].as_str().unwrap_or_default();
    assert!(
        hover.contains("i32"),
        "hover should describe the binding's type, got: {hover:?}"
    );

    pool.shutdown_all().await;
}

/// rust-analyzer needs time to index before it answers; poll rather than
/// asserting on the first try.
async fn wait_for_hover(
    pool: &Arc<LspPool>,
    file: &Path,
    project: &Path,
    source: &str,
    line: i64,
    col: i64,
) -> serde_json::Value {
    let mut last: serde_json::Value = serde_json::Value::Null;
    for _ in 0..30 {
        last = super::api::hover(pool, file, project, line, col, Some(source)).await;
        if last["hover"].as_str().is_some_and(|h| !h.is_empty()) {
            return last;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    last
}

/// A deliberate type error must come back as a diagnostic at the right line.
#[tokio::test]
#[ignore = "spawns a real rust-analyzer"]
async fn diagnostics_report_a_real_error() {
    if !have_rust_analyzer() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let dir = fixture();
    let file = dir.path().join("src/main.rs");
    let source = "fn main() {\n    let n: i32 = \"not a number\";\n    let _ = n;\n}\n";
    std::fs::write(&file, source).unwrap();

    let pool = Arc::new(LspPool::new());
    let mut found: serde_json::Value = serde_json::json!({});
    for _ in 0..40 {
        found = super::api::diagnostics(&pool, &file, dir.path(), Some(source)).await;
        if found["diagnostics"]
            .as_array()
            .is_some_and(|d| !d.is_empty())
        {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let diagnostics = found["diagnostics"].as_array().cloned().unwrap_or_default();
    assert!(
        !diagnostics.is_empty(),
        "a type error should produce a diagnostic, got: {found}"
    );
    assert_eq!(diagnostics[0]["lnum"], 2, "error is on the second line");
    assert_eq!(diagnostics[0]["severity"], "error");

    pool.shutdown_all().await;
}

/// Completion is the headline addition: `.` after a Vec must offer its methods.
#[tokio::test]
#[ignore = "spawns a real rust-analyzer"]
async fn completion_offers_methods_after_a_dot() {
    if !have_rust_analyzer() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let dir = fixture();
    let file = dir.path().join("src/main.rs");
    let source = "fn main() {\n    let items: Vec<i32> = Vec::new();\n    items.\n}\n";
    std::fs::write(&file, source).unwrap();

    let pool = Arc::new(LspPool::new());
    let mut found = serde_json::json!({});
    // Column 11 on line 3 is just after `items.`.
    for _ in 0..30 {
        found =
            super::api::completion(&pool, &file, dir.path(), 3, 11, Some(source), Some(".")).await;
        if found["items"].as_array().is_some_and(|i| !i.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert_eq!(
        found["available"], true,
        "completion should be available: {found}"
    );
    let labels: Vec<String> = found["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i["label"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        labels.iter().any(|l| l.starts_with("push")),
        "Vec methods should be offered, got: {:?}",
        &labels[..labels.len().min(15)]
    );

    // Trigger characters must reach the editor, or it never re-queries on `.`.
    let triggers = found["triggerCharacters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        triggers.iter().any(|t| t == "."),
        "rust-analyzer reports `.` as a trigger, got: {triggers:?}"
    );

    pool.shutdown_all().await;
}

/// Two files in one crate must share a single server — the whole reason the
/// pool is keyed by root rather than by file.
#[tokio::test]
#[ignore = "spawns a real rust-analyzer"]
async fn files_in_one_crate_share_one_server() {
    if !have_rust_analyzer() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let dir = fixture();
    let main = dir.path().join("src/main.rs");
    let other = dir.path().join("src/helper.rs");
    std::fs::write(&main, "mod helper;\nfn main() {}\n").unwrap();
    std::fs::write(&other, "pub fn help() {}\n").unwrap();

    let pool = Arc::new(LspPool::new());
    let a = pool.resolve(&main, dir.path()).await.unwrap().unwrap();
    let b = pool.resolve(&other, dir.path()).await.unwrap().unwrap();

    assert_eq!(a.key, b.key, "same crate must resolve to the same server");
    assert!(
        Arc::ptr_eq(&a.server, &b.server),
        "and to the very same process"
    );

    pool.shutdown_all().await;
}
