//! Kanban CRUD backed by SQLite: boards, tasks, attachments, notes.

use rusqlite::{params, OptionalExtension};

use super::Db;
use crate::web::types::*;

impl Db {
    // ── Boards ──────────────────────────────────────────────────────

    /// Fetch the board for a project path, if it exists.
    pub fn kanban_board_for_project(&self, project_path: &str) -> Option<Board> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, name, project_path, lanes, transitions
             FROM kanban_boards WHERE project_path = ?1",
            params![project_path],
            row_to_board,
        )
        .optional()
        .ok()
        .flatten()
    }

    /// Fetch a board by id.
    pub fn kanban_board(&self, board_id: &str) -> Option<Board> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, name, project_path, lanes, transitions
             FROM kanban_boards WHERE id = ?1",
            params![board_id],
            row_to_board,
        )
        .optional()
        .ok()
        .flatten()
    }

    /// Insert a board.
    pub fn insert_kanban_board(&self, board: &Board, now: &str) {
        let conn = self.conn();
        let lanes = serde_json::to_string(&board.lanes).unwrap_or_else(|_| "[]".into());
        let transitions =
            serde_json::to_string(&board.transitions).unwrap_or_else(|_| "{}".into());
        let _ = conn.execute(
            "INSERT INTO kanban_boards (id, project_path, name, lanes, transitions, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![board.id, board.project_path, board.name, lanes, transitions, now],
        );
    }

    /// Replace a board's lanes + transition graph.
    pub fn update_kanban_board_config(&self, board: &Board, now: &str) -> bool {
        let conn = self.conn();
        let lanes = serde_json::to_string(&board.lanes).unwrap_or_else(|_| "[]".into());
        let transitions =
            serde_json::to_string(&board.transitions).unwrap_or_else(|_| "{}".into());
        conn.execute(
            "UPDATE kanban_boards SET name=?2, lanes=?3, transitions=?4, updated_at=?5 WHERE id=?1",
            params![board.id, board.name, lanes, transitions, now],
        )
        .unwrap_or(0)
            > 0
    }

    // ── Tasks ───────────────────────────────────────────────────────

    /// All tasks on a board, ordered by lane then order_index.
    pub fn kanban_tasks_for_board(&self, board_id: &str) -> Vec<Task> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(TASK_SELECT) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![board_id], row_to_task)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Single task by id.
    pub fn kanban_task(&self, id: &str) -> Option<Task> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, board_id, lane_id, title, description, tags, priority, order_index,
                    session_id, launch_model, launch_agent, run_state, created_at, updated_at
             FROM kanban_tasks WHERE id = ?1",
            params![id],
            row_to_task,
        )
        .optional()
        .ok()
        .flatten()
    }

    /// Highest order_index currently in a lane (for appending to the end).
    pub fn kanban_max_order(&self, board_id: &str, lane_id: &str) -> f64 {
        let conn = self.conn();
        conn.query_row(
            "SELECT COALESCE(MAX(order_index), 0) FROM kanban_tasks WHERE board_id=?1 AND lane_id=?2",
            params![board_id, lane_id],
            |r| r.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn insert_kanban_task(&self, t: &Task) {
        let conn = self.conn();
        let tags = serde_json::to_string(&t.tags).unwrap_or_else(|_| "[]".into());
        let _ = conn.execute(
            "INSERT INTO kanban_tasks
                (id, board_id, lane_id, title, description, tags, priority, order_index,
                 session_id, launch_model, launch_agent, run_state, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                t.id, t.board_id, t.lane_id, t.title, t.description, tags, t.priority,
                t.order_index, t.session_id, t.launch_model, t.launch_agent, t.run_state,
                t.created_at, t.updated_at
            ],
        );
    }

    /// Persist a full task row (used for edits, moves, launch state changes).
    pub fn update_kanban_task(&self, t: &Task) -> bool {
        let conn = self.conn();
        let tags = serde_json::to_string(&t.tags).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "UPDATE kanban_tasks SET lane_id=?2, title=?3, description=?4, tags=?5, priority=?6,
                 order_index=?7, session_id=?8, launch_model=?9, launch_agent=?10, run_state=?11,
                 updated_at=?12
             WHERE id=?1",
            params![
                t.id, t.lane_id, t.title, t.description, tags, t.priority, t.order_index,
                t.session_id, t.launch_model, t.launch_agent, t.run_state, t.updated_at
            ],
        )
        .unwrap_or(0)
            > 0
    }

    pub fn delete_kanban_task(&self, id: &str) -> bool {
        let conn = self.conn();
        conn.execute("DELETE FROM kanban_tasks WHERE id=?1", params![id])
            .unwrap_or(0)
            > 0
    }

    // ── Attachments ─────────────────────────────────────────────────

    pub fn insert_kanban_attachment(&self, a: &Attachment) {
        let conn = self.conn();
        let _ = conn.execute(
            "INSERT INTO kanban_attachments (id, task_id, filename, mime, size_bytes, kind, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![a.id, a.task_id, a.filename, a.mime, a.size_bytes, a.kind, a.created_at],
        );
    }

    pub fn kanban_attachments_for_task(&self, task_id: &str) -> Vec<Attachment> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT id, task_id, filename, mime, size_bytes, kind, created_at
             FROM kanban_attachments WHERE task_id=?1 ORDER BY created_at ASC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![task_id], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                task_id: row.get(1)?,
                filename: row.get(2)?,
                mime: row.get(3)?,
                size_bytes: row.get(4)?,
                kind: row.get(5)?,
                created_at: row.get(6)?,
                url: String::new(),
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // ── Notes ───────────────────────────────────────────────────────

    pub fn insert_kanban_note(&self, n: &KanbanNote, task_id: &str) {
        let conn = self.conn();
        let _ = conn.execute(
            "INSERT INTO kanban_notes (id, task_id, author, body, lane_from, lane_to, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![n.id, task_id, n.author, n.body, n.lane_from, n.lane_to, n.created_at],
        );
    }

    pub fn kanban_notes_for_task(&self, task_id: &str) -> Vec<KanbanNote> {
        let conn = self.conn();
        let mut stmt = match conn.prepare(
            "SELECT id, author, body, lane_from, lane_to, created_at
             FROM kanban_notes WHERE task_id=?1 ORDER BY created_at ASC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![task_id], |row| {
            Ok(KanbanNote {
                id: row.get(0)?,
                author: row.get(1)?,
                body: row.get(2)?,
                lane_from: row.get(3)?,
                lane_to: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }
}

const TASK_SELECT: &str = "SELECT id, board_id, lane_id, title, description, tags, priority,
        order_index, session_id, launch_model, launch_agent, run_state, created_at, updated_at
     FROM kanban_tasks WHERE board_id = ?1 ORDER BY lane_id, order_index ASC";

fn row_to_board(row: &rusqlite::Row) -> rusqlite::Result<Board> {
    let lanes_json: String = row.get(3)?;
    let transitions_json: String = row.get(4)?;
    Ok(Board {
        id: row.get(0)?,
        name: row.get(1)?,
        project_path: row.get(2)?,
        lanes: serde_json::from_str(&lanes_json).unwrap_or_default(),
        transitions: serde_json::from_str(&transitions_json).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::types::default_board;

    #[test]
    fn board_and_task_round_trip() {
        let db = Db::open_memory().unwrap();
        let board = default_board("brd_1".into(), "/proj".into());
        db.insert_kanban_board(&board, "2026-01-01T00:00:00Z");

        // Lazily-fetched board matches.
        let fetched = db.kanban_board_for_project("/proj").unwrap();
        assert_eq!(fetched.id, "brd_1");
        assert_eq!(fetched.lanes.len(), 7);
        // Default graph: Todo → Planning is allowed; Todo → Done is not.
        assert!(fetched.transition_allowed("lane_todo", "lane_planning"));
        assert!(!fetched.transition_allowed("lane_todo", "lane_done"));
        // In Review is the terminal lane.
        assert_eq!(fetched.terminal_lane_id(), Some("lane_inreview"));

        let task = Task {
            id: "tsk_1".into(),
            board_id: "brd_1".into(),
            lane_id: "lane_todo".into(),
            title: "Do the thing".into(),
            description: "details".into(),
            tags: vec!["a".into(), "b".into()],
            priority: "high".into(),
            order_index: 1.0,
            session_id: None,
            launch_model: None,
            launch_agent: None,
            run_state: "idle".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        db.insert_kanban_task(&task);
        let tasks = db.kanban_tasks_for_board("brd_1");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].tags, vec!["a", "b"]);
        assert_eq!(tasks[0].priority, "high");

        // Note + attachment round-trip.
        db.insert_kanban_note(
            &KanbanNote {
                id: "nte_1".into(),
                author: "agent".into(),
                body: "started".into(),
                lane_from: Some("lane_todo".into()),
                lane_to: Some("lane_planning".into()),
                created_at: "2026-01-01T00:01:00Z".into(),
            },
            "tsk_1",
        );
        assert_eq!(db.kanban_notes_for_task("tsk_1").len(), 1);

        assert!(db.delete_kanban_task("tsk_1"));
        assert!(db.kanban_tasks_for_board("brd_1").is_empty());
    }
}

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let tags_json: String = row.get(5)?;
    Ok(Task {
        id: row.get(0)?,
        board_id: row.get(1)?,
        lane_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        priority: row.get(6)?,
        order_index: row.get(7)?,
        session_id: row.get(8)?,
        launch_model: row.get(9)?,
        launch_agent: row.get(10)?,
        run_state: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}
