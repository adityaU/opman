//! DDL: table creation for the assistant database.

use rusqlite::Connection;

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

        -- ── Routines ────────────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS routines (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            trigger    TEXT NOT NULL DEFAULT 'manual',
            action     TEXT NOT NULL DEFAULT 'review_mission',
            mission_id TEXT,
            session_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- ── Routine Runs ────────────────────────────────────────────
        CREATE TABLE IF NOT EXISTS routine_runs (
            id         TEXT PRIMARY KEY,
            routine_id TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'completed',
            summary    TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
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

        -- ── Indexes ─────────────────────────────────────────────────
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
        ",
    )?;
    Ok(())
}
