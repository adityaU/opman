//! opencode's SQLite layout, built in a temp dir with the columns the reader touches.

use super::*;

use rusqlite::Connection;

/// One `(id, project, parent, directory, title, updated)` row.
type Row<'a> = (&'a str, &'a str, Option<&'a str>, &'a str, &'a str, i64);

/// A database holding `rows`.
fn fixture(rows: &[Row<'_>]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join(DEFAULT_DB);
    let conn = Connection::open(&db).expect("open");
    conn.execute_batch(
        "CREATE TABLE session (
            id TEXT PRIMARY KEY, project_id TEXT NOT NULL, parent_id TEXT,
            slug TEXT NOT NULL DEFAULT '', directory TEXT NOT NULL, title TEXT NOT NULL,
            version TEXT NOT NULL DEFAULT '1', time_created INTEGER NOT NULL,
            time_updated INTEGER NOT NULL, time_archived INTEGER)",
    )
    .expect("schema");
    for (id, project, parent, directory, title, updated) in rows {
        conn.execute(
            "INSERT INTO session (id, project_id, parent_id, directory, title, time_created, time_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, project, parent, directory, title, updated - 1, updated],
        )
        .expect("insert");
    }
    (dir, db)
}

#[test]
fn lists_the_directory_newest_first_with_metadata() {
    let (_dir, db) = fixture(&[
        ("ses_old", "p1", None, "/proj", "Old", 10),
        ("ses_new", "p1", None, "/proj", "New", 30),
        ("ses_child", "p1", Some("ses_new"), "/proj", "Child", 20),
        ("ses_other", "p2", None, "/elsewhere", "Other", 40),
    ]);
    let sessions = list(&db, "/proj", 100).expect("list");
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["ses_new", "ses_child", "ses_old"]);
    assert_eq!(sessions[0].title, "New");
    assert_eq!(sessions[0].time.created, 29);
    assert_eq!(sessions[0].time.updated, 30);
    assert_eq!(sessions[0].parent_id, "");
    assert_eq!(sessions[1].parent_id, "ses_new");
}

#[test]
fn the_project_limit_applies_before_the_directory_filter_like_opencode() {
    // opencode returns the project's newest `limit` sessions; opman keeps its directory's.
    // A newer session in a sibling directory of the same project takes a slot.
    let (_dir, db) = fixture(&[
        ("ses_a", "p1", None, "/proj", "A", 10),
        ("ses_b", "p1", None, "/proj/sub", "B", 30),
        ("ses_c", "p1", None, "/proj", "C", 20),
    ]);
    let sessions = list(&db, "/proj", 2).expect("list");
    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["ses_c"]);
}

#[test]
fn an_unknown_directory_lists_nothing() {
    let (_dir, db) = fixture(&[("ses_a", "p1", None, "/proj", "A", 10)]);
    assert!(list(&db, "/nowhere", 100).expect("list").is_empty());
}

#[test]
fn reading_never_writes_the_database() {
    let (_dir, db) = fixture(&[("ses_a", "p1", None, "/proj", "A", 10)]);
    let before = std::fs::read(&db).expect("read");
    let mut perms = std::fs::metadata(&db).expect("meta").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&db, perms).expect("chmod");
    assert_eq!(list(&db, "/proj", 100).expect("list").len(), 1);
    assert_eq!(std::fs::read(&db).expect("read"), before);
}

#[test]
fn database_prefers_the_default_channel_then_the_newest_other() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(database(dir.path()), None);
    std::fs::write(dir.path().join("opencode-beta.db"), b"").expect("write");
    assert_eq!(
        database(dir.path()),
        Some(dir.path().join("opencode-beta.db"))
    );
    std::fs::write(dir.path().join(DEFAULT_DB), b"").expect("write");
    assert_eq!(database(dir.path()), Some(dir.path().join(DEFAULT_DB)));
}
