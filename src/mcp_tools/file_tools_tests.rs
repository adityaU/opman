use super::edit_file::{run as edit, EditArgs};
use super::list_dir::{run as list, ListArgs};
use super::read_file::{run as read, ReadArgs};
use super::workspace::Workspace;
use super::write_file::{run as write, WriteArgs};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

fn from<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("test fixture or assertion")
}

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("test fixture or assertion");
    let ws = Workspace::new(dir.path().to_path_buf());
    (dir, ws)
}

#[tokio::test]
async fn write_creates_parents_then_overwrites() {
    let (dir, ws) = ws();
    let first = write(
        &ws,
        from::<WriteArgs>(json!({"path": "a/b/c.txt", "content": "one"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(first.data["created"], true);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a/b/c.txt")).expect("test fixture or assertion"),
        "one"
    );
    let second = write(
        &ws,
        from::<WriteArgs>(json!({"path": "a/b/c.txt", "content": "two!"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(second.data["created"], false);
    assert_eq!(second.data["bytes"], 4);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("a/b"))
        .expect("test fixture or assertion")
        .collect();
    assert_eq!(leftovers.len(), 1, "temp file left behind");
}

#[tokio::test]
async fn write_to_a_directory_fails() {
    let (dir, ws) = ws();
    std::fs::create_dir(dir.path().join("d")).expect("test fixture or assertion");
    let err = write(&ws, from::<WriteArgs>(json!({"path": "d", "content": "x"})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(err.message().contains("is a directory"));
}

#[tokio::test]
async fn read_numbers_lines_and_pages() {
    let (dir, ws) = ws();
    let body: String = (1..=10).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.path().join("f.txt"), body).expect("test fixture or assertion");
    let out = read(
        &ws,
        from::<ReadArgs>(json!({"path": "f.txt", "offset": 3, "limit": 2})),
    )
    .await
    .expect("test fixture or assertion");
    assert!(
        out.text.starts_with("     3\tline 3\n     4\tline 4\n"),
        "{}",
        out.text
    );
    assert!(out.text.contains("offset 5"));
    assert_eq!(out.data["start_line"], 3);
    assert_eq!(out.data["end_line"], 4);
    assert_eq!(out.data["has_more"], true);
    assert_eq!(out.data["content"], "line 3\nline 4\n");
}

#[tokio::test]
async fn read_errors_are_clear() {
    let (dir, ws) = ws();
    let missing = read(&ws, from::<ReadArgs>(json!({"path": "nope.txt"})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(missing.message().contains("no such file"), "{missing}");
    std::fs::write(dir.path().join("bin"), [1u8, 0, 2, 3]).expect("test fixture or assertion");
    let binary = read(&ws, from::<ReadArgs>(json!({"path": "bin"})))
        .await
        .expect("test fixture or assertion");
    assert!(binary.text.contains("binary"));
    assert!(!binary.is_error);
    assert_eq!(binary.data["binary"], true);
    assert_eq!(binary.data["bytes"], 4);
    assert_eq!(binary.data["path"], "bin");
    std::fs::write(dir.path().join("s.txt"), "a\nb\n").expect("test fixture or assertion");
    let past = read(&ws, from::<ReadArgs>(json!({"path": "s.txt", "offset": 9})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(past.message().contains("past the end"));
    let folder = read(&ws, from::<ReadArgs>(json!({"path": "."})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(folder.message().contains("list_dir"));
}

#[tokio::test]
async fn read_empty_file_and_cut_long_lines() {
    let (dir, ws) = ws();
    std::fs::write(dir.path().join("e.txt"), "").expect("test fixture or assertion");
    let empty = read(&ws, from::<ReadArgs>(json!({"path": "e.txt"})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(empty.text, "(empty file)");
    std::fs::write(dir.path().join("l.txt"), "x".repeat(5000)).expect("test fixture or assertion");
    let long = read(&ws, from::<ReadArgs>(json!({"path": "l.txt"})))
        .await
        .expect("test fixture or assertion");
    assert_eq!(long.data["long_lines_clipped"], true);
    assert!(long.text.len() < 2100);
}

#[tokio::test]
async fn edit_replaces_one_exact_match_and_shows_a_diff() {
    let (dir, ws) = ws();
    let file = dir.path().join("e.rs");
    std::fs::write(&file, "fn a() {}\nfn b() {}\n").expect("test fixture or assertion");
    let out = edit(
        &ws,
        from::<EditArgs>(
            json!({"path": "e.rs", "old_string": "fn b() {}", "new_string": "fn c() {}"}),
        ),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(
        std::fs::read_to_string(&file).expect("test fixture or assertion"),
        "fn a() {}\nfn c() {}\n"
    );
    assert_eq!(out.data["replacements"], 1);
    assert_eq!(out.data["first_line"], 2);
    let diff = out.data["diff"]
        .as_str()
        .expect("test fixture or assertion");
    assert!(
        diff.contains("-fn b() {}") && diff.contains("+fn c() {}"),
        "{diff}"
    );
}

#[tokio::test]
async fn edit_refuses_missing_and_non_unique_matches() {
    let (dir, ws) = ws();
    let file = dir.path().join("e.txt");
    std::fs::write(&file, "x\nx\n").expect("test fixture or assertion");
    let many = edit(
        &ws,
        from::<EditArgs>(json!({"path": "e.txt", "old_string": "x", "new_string": "y"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(many.message().contains("2 times"), "{many}");
    assert_eq!(
        std::fs::read_to_string(&file).expect("test fixture or assertion"),
        "x\nx\n"
    );
    let none = edit(
        &ws,
        from::<EditArgs>(json!({"path": "e.txt", "old_string": "zzz", "new_string": "y"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(none.message().contains("not found"));
    let same = edit(
        &ws,
        from::<EditArgs>(json!({"path": "e.txt", "old_string": "x", "new_string": "x"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(same.message().contains("same"));
    let nofile = edit(
        &ws,
        from::<EditArgs>(json!({"path": "q.txt", "old_string": "x", "new_string": "y"})),
    )
    .await
    .err()
    .expect("test fixture or assertion");
    assert!(nofile.message().contains("no such file"));
    let all = edit(
        &ws,
        from::<EditArgs>(
            json!({"path": "e.txt", "old_string": "x", "new_string": "y", "replace_all": true}),
        ),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(all.data["replacements"], 2);
    assert_eq!(
        std::fs::read_to_string(&file).expect("test fixture or assertion"),
        "y\ny\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn edit_keeps_the_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, ws) = ws();
    let file = dir.path().join("run.sh");
    std::fs::write(&file, "echo a\n").expect("test fixture or assertion");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))
        .expect("test fixture or assertion");
    edit(
        &ws,
        from::<EditArgs>(json!({"path": "run.sh", "old_string": "a", "new_string": "b"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(
        std::fs::metadata(&file)
            .expect("test fixture or assertion")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[tokio::test]
async fn list_dir_puts_folders_first_and_flags_errors() {
    let (dir, ws) = ws();
    std::fs::create_dir(dir.path().join("zdir")).expect("test fixture or assertion");
    std::fs::write(dir.path().join("a.txt"), "12345").expect("test fixture or assertion");
    let out = list(&ws, ListArgs::default())
        .await
        .expect("test fixture or assertion");
    assert_eq!(out.text, "zdir/\na.txt\n");
    assert_eq!(out.data["entries"][1]["size"], 5);
    let gone = list(&ws, from::<ListArgs>(json!({"path": "nope"})))
        .await
        .err()
        .expect("test fixture or assertion");
    assert!(gone.message().contains("no such file"));
}

#[tokio::test]
async fn paths_outside_the_project_are_allowed() {
    let (_dir, ws) = ws();
    let other = tempfile::tempdir().expect("test fixture or assertion");
    let target = other.path().join("o.txt");
    write(
        &ws,
        from::<WriteArgs>(json!({"path": target.display().to_string(), "content": "out"})),
    )
    .await
    .expect("test fixture or assertion");
    assert_eq!(
        std::fs::read_to_string(target).expect("test fixture or assertion"),
        "out"
    );
}
