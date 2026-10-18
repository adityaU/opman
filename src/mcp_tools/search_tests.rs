use super::glob::{run as glob, GlobArgs};
use super::grep::{run as grep, GrepArgs};
use super::workspace::Workspace;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

fn from<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("test fixture or assertion")
}

fn project() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("test fixture or assertion");
    let root = dir.path();
    std::fs::create_dir_all(root.join("src/deep")).expect("test fixture or assertion");
    std::fs::create_dir_all(root.join("target")).expect("test fixture or assertion");
    std::fs::create_dir_all(root.join(".git")).expect("test fixture or assertion");
    std::fs::write(root.join(".gitignore"), "target/\n").expect("test fixture or assertion");
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    println!(\"Hello\");\n}\n",
    )
    .expect("test fixture or assertion");
    std::fs::write(
        root.join("src/deep/lib.rs"),
        "// hello again\npub fn hello() {}\n",
    )
    .expect("test fixture or assertion");
    std::fs::write(root.join("src/notes.md"), "hello docs\n").expect("test fixture or assertion");
    std::fs::write(root.join("target/junk.rs"), "hello junk\n").expect("test fixture or assertion");
    std::fs::write(root.join(".git/config"), "hello git\n").expect("test fixture or assertion");
    std::fs::write(
        root.join("blob.bin"),
        [0u8, 1, 2, b'h', b'e', b'l', b'l', b'o'],
    )
    .expect("test fixture or assertion");
    let ws = Workspace::new(root.to_path_buf());
    (dir, ws)
}

#[tokio::test]
async fn glob_matches_at_any_depth_and_honours_gitignore() {
    let (_dir, ws) = project();
    let out = glob(&ws, from::<GlobArgs>(json!({"pattern": "*.rs"})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(out.data["files"], json!(["src/deep/lib.rs", "src/main.rs"]));
    let scoped = glob(
        &ws,
        from::<GlobArgs>(json!({"pattern": "**/*.rs", "path": "src/deep"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(scoped.data["files"], json!(["lib.rs"]));
    let none = glob(&ws, from::<GlobArgs>(json!({"pattern": "*.zzz"})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(none.text, "No files matched");
}

#[tokio::test]
async fn glob_errors() {
    let (_dir, ws) = project();
    let missing = glob(
        &ws,
        from::<GlobArgs>(json!({"pattern": "*", "path": "nope"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(missing.message().contains("no such file"));
    let file = glob(
        &ws,
        from::<GlobArgs>(json!({"pattern": "*", "path": "src/main.rs"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(file.message().contains("not a folder"));
}

#[tokio::test]
async fn grep_lists_files_skipping_ignored_git_and_binary() {
    let (_dir, ws) = project();
    let out = grep(
        &ws,
        from::<GrepArgs>(json!({"pattern": "hello", "case": "insensitive"})),
    )
    .await
    .expect("test fixture or assertion");
    let paths: Vec<_> = out.data["results"]
        .as_array()
        .expect("test fixture or assertion")
        .iter()
        .map(|r| r["path"].as_str().expect("test fixture or assertion"))
        .collect();
    assert_eq!(paths, ["src/deep/lib.rs", "src/main.rs", "src/notes.md"]);
}

#[tokio::test]
async fn grep_case_modes() {
    let (_dir, ws) = project();
    let sensitive = grep(
        &ws,
        from::<GrepArgs>(json!({"pattern": "Hello", "output_mode": "count"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(sensitive.text, "src/main.rs:1");
    let smart = grep(
        &ws,
        from::<GrepArgs>(
            json!({"pattern": "hello", "case": "smart", "glob": "*.rs", "output_mode": "count"}),
        ),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(smart.text, "src/deep/lib.rs:2\nsrc/main.rs:1");
}

#[tokio::test]
async fn grep_content_with_context() {
    let (_dir, ws) = project();
    let out = grep(
        &ws,
        from::<GrepArgs>(json!({"pattern": "println", "output_mode": "content", "context": 1})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(
        out.text,
        "src/main.rs-1-fn main() {\nsrc/main.rs:2:    println!(\"Hello\");\nsrc/main.rs-3-}"
    );
    let items = out.data["results"]
        .as_array()
        .expect("test fixture or assertion");
    assert_eq!(items[1]["line"], 2);
    assert!(items[0].get("context").is_some() && items[1].get("context").is_none());
}

#[tokio::test]
async fn grep_limit_and_errors() {
    let (_dir, ws) = project();
    let limited = grep(
        &ws,
        from::<GrepArgs>(json!({"pattern": "hello", "case": "insensitive", "head_limit": 1})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(limited.data["truncated"], true);
    assert_eq!(
        limited.data["results"]
            .as_array()
            .expect("test fixture or assertion")
            .len(),
        1
    );
    let bad = grep(&ws, from::<GrepArgs>(json!({"pattern": "(["})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(bad.message().contains("invalid regex"));
    let nopath = grep(
        &ws,
        from::<GrepArgs>(json!({"pattern": "x", "path": "nope"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(nopath.message().contains("no such file"));
    let none = grep(&ws, from::<GrepArgs>(json!({"pattern": "zzzzzz"})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(none.text, "No matches found");
}

#[tokio::test]
async fn grep_a_single_file() {
    let (_dir, ws) = project();
    let out = grep(
        &ws,
        from::<GrepArgs>(json!({"pattern": "fn", "path": "src/main.rs", "output_mode": "content"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(out.text, "main.rs:1:fn main() {");
}
