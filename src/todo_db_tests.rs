use super::*;
use crate::app::TodoItem;
use std::ffi::OsString;
use std::sync::Mutex;
use tempfile::TempDir;

// Serialize HOME mutation across tests in this file.
#[allow(unused_imports)]
use crate::claude_engine::claude_cli::ENV_LOCK as HOME_LOCK;

fn home_lock() -> std::sync::MutexGuard<'static, ()> {
    HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

struct HomeGuard {
    old: Option<OsString>,
}

impl HomeGuard {
    fn set(path: &std::path::Path) -> Self {
        let old = std::env::var_os("HOME");
        std::env::set_var("HOME", path);
        HomeGuard { old }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// Create the opencode db directory under `home` and return the db path.
fn make_db_dir(home: &std::path::Path) -> std::path::PathBuf {
    let dir = home.join(".local/share/opencode");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("opencode.db")
}

fn create_todo_table(db_path: &std::path::Path) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute(
        "CREATE TABLE todo (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            content TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL,
            position INTEGER NOT NULL,
            time_created INTEGER NOT NULL,
            time_updated INTEGER NOT NULL
        )",
        [],
    )
    .unwrap();
}

fn item(content: &str, status: &str, priority: &str) -> TodoItem {
    TodoItem {
        content: content.to_string(),
        status: status.to_string(),
        priority: priority.to_string(),
    }
}

fn fetch_rows(db_path: &std::path::Path, session: &str) -> Vec<(String, String, String, i64)> {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    let mut stmt = conn
        .prepare("SELECT content, status, priority, position FROM todo WHERE session_id = ? ORDER BY position")
        .unwrap();
    let rows = stmt
        .query_map([session], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    rows
}

#[test]
fn save_todos_inserts_rows_in_order() {
    let _l = home_lock();
    let tmp = TempDir::new().unwrap();
    let db = make_db_dir(tmp.path());
    create_todo_table(&db);
    let _h = HomeGuard::set(tmp.path());

    let todos = vec![
        item("first", "pending", "high"),
        item("second", "completed", "low"),
    ];
    save_todos_to_db("sess1", &todos).unwrap();

    let rows = fetch_rows(&db, "sess1");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], ("first".into(), "pending".into(), "high".into(), 0));
    assert_eq!(
        rows[1],
        ("second".into(), "completed".into(), "low".into(), 1)
    );
}

#[test]
fn save_todos_full_replace_semantics() {
    let _l = home_lock();
    let tmp = TempDir::new().unwrap();
    let db = make_db_dir(tmp.path());
    create_todo_table(&db);
    let _h = HomeGuard::set(tmp.path());

    save_todos_to_db("s", &[item("a", "pending", "high"), item("b", "pending", "low")]).unwrap();
    // Second save replaces the first entirely.
    save_todos_to_db("s", &[item("only", "completed", "medium")]).unwrap();

    let rows = fetch_rows(&db, "s");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "only");
}

#[test]
fn save_todos_empty_clears_session_only() {
    let _l = home_lock();
    let tmp = TempDir::new().unwrap();
    let db = make_db_dir(tmp.path());
    create_todo_table(&db);
    let _h = HomeGuard::set(tmp.path());

    save_todos_to_db("keep", &[item("x", "pending", "high")]).unwrap();
    save_todos_to_db("drop", &[item("y", "pending", "high")]).unwrap();
    // Empty save for "drop" deletes its rows but leaves "keep" untouched.
    save_todos_to_db("drop", &[]).unwrap();

    assert_eq!(fetch_rows(&db, "drop").len(), 0);
    assert_eq!(fetch_rows(&db, "keep").len(), 1);
}

#[test]
fn save_todos_errors_when_table_missing() {
    let _l = home_lock();
    let tmp = TempDir::new().unwrap();
    // Create the directory + an empty db WITHOUT the todo table.
    let db = make_db_dir(tmp.path());
    rusqlite::Connection::open(&db).unwrap(); // creates empty db file
    let _h = HomeGuard::set(tmp.path());

    let res = save_todos_to_db("sess", &[item("a", "pending", "high")]);
    assert!(res.is_err());
}

#[test]
fn save_todos_errors_when_db_dir_missing() {
    let _l = home_lock();
    let tmp = TempDir::new().unwrap();
    // Do NOT create .local/share/opencode -> Connection::open cannot create file.
    let _h = HomeGuard::set(tmp.path());
    let res = save_todos_to_db("sess", &[item("a", "pending", "high")]);
    assert!(res.is_err());
}
