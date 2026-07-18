//! DDL: table creation for the assistant database.

use rusqlite::Connection;

/// Create all tables (IF NOT EXISTS). Safe to call on existing databases
/// — will not alter existing table schemas (use migrations for that).
pub(super) fn create_tables(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        -- ── Missions (v2: goal-driven loop) ─────────────────────────────
        CREATE TABLE IF NOT EXISTS missions (
            id              TEXT PRIMARY KEY,
            goal            TEXT NOT NULL DEFAULT '',
            session_id      TEXT NOT NULL DEFAULT '',
            project_index   INTEGER NOT NULL DEFAULT 0,
            state           TEXT NOT NULL DEFAULT 'pending',
            iteration       INTEGER NOT NULL DEFAULT 0,
            max_iterations  INTEGER NOT NULL DEFAULT 10,
            last_verdict    TEXT,
            last_eval_summary TEXT,
            eval_history    TEXT NOT NULL DEFAULT '[]',
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        -- ── Personal Memory ─────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS personal_memory (
            id            TEXT PRIMARY KEY,
            label         TEXT NOT NULL,
            content       TEXT NOT NULL DEFAULT '',
            scope         TEXT NOT NULL DEFAULT 'global',
            project_index INTEGER,
            session_id    TEXT,
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
        );

        -- ── Autonomy Settings ───────────────────────────────────────
        CREATE TABLE IF NOT EXISTS autonomy_settings (
            id         INTEGER PRIMARY KEY CHECK (id = 1),
            mode       TEXT NOT NULL DEFAULT 'observe',
            updated_at TEXT NOT NULL
        );

        -- ── Routines (v2: message-dispatch automation) ──────────────
        CREATE TABLE IF NOT EXISTS routines (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            trigger       TEXT NOT NULL DEFAULT 'manual',
            action        TEXT NOT NULL DEFAULT 'send_message',
            enabled       INTEGER NOT NULL DEFAULT 1,
            cron_expr     TEXT,
            timezone      TEXT,
            target_mode   TEXT,
            session_id    TEXT,
            project_index INTEGER,
            prompt        TEXT,
            provider_id   TEXT,
            model_id      TEXT,
            mission_id    TEXT,
            last_run_at   TEXT,
            next_run_at   TEXT,
            last_error    TEXT,
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
        );

        -- ── Routine Runs ────────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS routine_runs (
            id                TEXT PRIMARY KEY,
            routine_id        TEXT NOT NULL,
            status            TEXT NOT NULL DEFAULT 'completed',
            summary           TEXT NOT NULL DEFAULT '',
            target_session_id TEXT,
            duration_ms       INTEGER,
            created_at        TEXT NOT NULL
        );

        -- ── Delegated Work ──────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS delegated_work (
            id                  TEXT PRIMARY KEY,
            title               TEXT NOT NULL,
            assignee            TEXT NOT NULL DEFAULT '',
            scope               TEXT NOT NULL DEFAULT '',
            status              TEXT NOT NULL DEFAULT 'planned',
            mission_id          TEXT,
            session_id          TEXT,
            subagent_session_id TEXT,
            created_at          TEXT NOT NULL,
            updated_at          TEXT NOT NULL
        );

        -- ── Workspace Snapshots ─────────────────────────────────────
        CREATE TABLE IF NOT EXISTS workspaces (
            name         TEXT PRIMARY KEY,
            snapshot     TEXT NOT NULL,
            created_at   TEXT NOT NULL
        );

        -- ── Signals ─────────────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS signals (
            id         TEXT PRIMARY KEY,
            kind       TEXT NOT NULL DEFAULT '',
            title      TEXT NOT NULL DEFAULT '',
            body       TEXT NOT NULL DEFAULT '',
            created_at REAL NOT NULL DEFAULT 0,
            session_id TEXT
        );

        -- ── Kanban: boards (one per project) ────────────────────────
        CREATE TABLE IF NOT EXISTS kanban_boards (
            id            TEXT PRIMARY KEY,
            project_path  TEXT NOT NULL UNIQUE,
            name          TEXT NOT NULL DEFAULT 'Board',
            lanes         TEXT NOT NULL DEFAULT '[]',
            transitions   TEXT NOT NULL DEFAULT '{}',
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
        );

        -- ── Kanban: tasks ───────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS kanban_tasks (
            id            TEXT PRIMARY KEY,
            board_id      TEXT NOT NULL REFERENCES kanban_boards(id) ON DELETE CASCADE,
            lane_id       TEXT NOT NULL,
            title         TEXT NOT NULL,
            description   TEXT NOT NULL DEFAULT '',
            tags          TEXT NOT NULL DEFAULT '[]',
            priority      TEXT NOT NULL DEFAULT 'normal',
            order_index   REAL NOT NULL DEFAULT 0,
            session_id    TEXT,
            launch_model  TEXT,
            launch_agent  TEXT,
            run_state     TEXT NOT NULL DEFAULT 'idle',
            archived      INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
        );

        -- ── Kanban: attachments (metadata; binaries live on disk) ───
        CREATE TABLE IF NOT EXISTS kanban_attachments (
            id            TEXT PRIMARY KEY,
            task_id       TEXT NOT NULL REFERENCES kanban_tasks(id) ON DELETE CASCADE,
            filename      TEXT NOT NULL,
            mime          TEXT NOT NULL DEFAULT '',
            size_bytes    INTEGER NOT NULL DEFAULT 0,
            kind          TEXT NOT NULL DEFAULT 'file',
            created_at    TEXT NOT NULL
        );

        -- ── Kanban: notes (agent/user progress log) ─────────────────
        CREATE TABLE IF NOT EXISTS kanban_notes (
            id            TEXT PRIMARY KEY,
            task_id       TEXT NOT NULL REFERENCES kanban_tasks(id) ON DELETE CASCADE,
            author        TEXT NOT NULL DEFAULT 'agent',
            body          TEXT NOT NULL DEFAULT '',
            lane_from     TEXT,
            lane_to       TEXT,
            created_at    TEXT NOT NULL
        );

        -- ── Kanban: pipeline runs (staged, multi-session launches) ──
        CREATE TABLE IF NOT EXISTS kanban_pipeline_runs (
            task_id       TEXT PRIMARY KEY REFERENCES kanban_tasks(id) ON DELETE CASCADE,
            stages        TEXT NOT NULL DEFAULT '[]',
            current_index INTEGER NOT NULL DEFAULT 0,
            status        TEXT NOT NULL DEFAULT 'running',
            launch_model  TEXT,
            launch_agent  TEXT,
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
        );
        ",
    )?;
    Ok(())
}

/// Create indexes. Called AFTER schema migrations have ensured all columns
/// exist, so column references are guaranteed valid.
pub(super) fn create_indexes(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_missions_state
            ON missions(state);
        CREATE INDEX IF NOT EXISTS idx_missions_session
            ON missions(session_id);
        CREATE INDEX IF NOT EXISTS idx_memory_scope
            ON personal_memory(scope);
        CREATE INDEX IF NOT EXISTS idx_routine_runs_routine
            ON routine_runs(routine_id);
        CREATE INDEX IF NOT EXISTS idx_delegated_status
            ON delegated_work(status);
        CREATE INDEX IF NOT EXISTS idx_signals_created
            ON signals(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_kanban_tasks_board
            ON kanban_tasks(board_id);
        CREATE INDEX IF NOT EXISTS idx_kanban_tasks_session
            ON kanban_tasks(session_id);
        CREATE INDEX IF NOT EXISTS idx_kanban_attachments_task
            ON kanban_attachments(task_id);
        CREATE INDEX IF NOT EXISTS idx_kanban_notes_task
            ON kanban_notes(task_id);
        ",
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod schema_tests;
