use crate::app::TodoItem;
use anyhow::Result;

/// Save todos to the opencode SQLite database using full-replace semantics.
/// Deletes all existing todos for the session, then inserts the new list.
pub fn save_todos_to_db(session_id: &str, todos: &[TodoItem]) -> Result<()> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".local/share/opencode/opencode.db");
    let conn = rusqlite::Connection::open(&db_path)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM todo WHERE session_id = ?", [session_id])?;
    for (i, todo) in todos.iter().enumerate() {
        tx.execute(
            "INSERT INTO todo (session_id, content, status, priority, position, time_created, time_updated) VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![session_id, todo.content, todo.status, todo.priority, i as i64, now, now],
        )?;
    }
    tx.commit()?;
    Ok(())
}
