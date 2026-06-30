import React, { useMemo } from "react";
import { Bot, UserRound, ArrowRight, Flag, Tag, Layers } from "lucide-react";
import { asObj, str, shortId } from "./KanbanToolCard";

// ── Kanban query-tool bodies (read_notes / list_tasks / board_summary) ──
//
// The read/query kanban tools return rich structured output; these render it
// as compact themed lists instead of dumping raw JSON. Theme-aware: every
// colour resolves from CSS custom properties so glassy and flat both work.

const asArr = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
const num = (v: unknown): number | null => (typeof v === "number" ? v : null);

function fmtTime(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

// ── read_notes: the activity timeline of one or more tasks ─────────────

interface RawNote {
  author?: string;
  body?: string;
  lane_from?: string | null;
  lane_to?: string | null;
  created_at?: string;
}

function NoteEntry({ note }: { note: RawNote }) {
  const isUser = note.author === "user";
  const moved =
    note.lane_from && note.lane_to && note.lane_from !== note.lane_to;
  return (
    <li className={`kqb-note ${isUser ? "kqb-note-user" : ""}`}>
      <span className="kqb-note-icon">
        {isUser ? <UserRound size={11} /> : <Bot size={11} />}
      </span>
      <div className="kqb-note-main">
        <div className="kqb-note-head">
          <span className="kqb-note-author">{isUser ? "You" : "Agent"}</span>
          {moved && (
            <span className="kqb-note-move">
              {shortId(str(note.lane_from))}
              <ArrowRight size={9} aria-hidden />
              {shortId(str(note.lane_to))}
            </span>
          )}
          {note.created_at && (
            <span className="kqb-note-time">{fmtTime(str(note.created_at))}</span>
          )}
        </div>
        {note.body && <div className="kqb-note-body">{note.body}</div>}
      </div>
    </li>
  );
}

export function ReadNotesBody({ output, isRunning }: { output: string | null; isRunning: boolean }) {
  const tasks = useMemo(() => asArr(asObj(output).tasks), [output]);
  if (!output) {
    return <div className="kanban-card-muted">{isRunning ? "Reading notes…" : "No notes"}</div>;
  }
  const totalNotes = tasks.reduce((acc: number, t) => acc + asArr(asObj(t).notes).length, 0);
  if (totalNotes === 0) return <div className="kanban-card-muted">No notes yet</div>;

  return (
    <>
      {tasks.map((t, ti) => {
        const task = asObj(t);
        const notes = asArr(task.notes) as RawNote[];
        if (notes.length === 0) return null;
        const title = str(task.title) || shortId(str(task.id));
        return (
          <div key={str(task.id) || ti} className="kqb-note-group">
            {tasks.length > 1 && title && (
              <div className="kqb-section-head">{title} · {notes.length}</div>
            )}
            <ul className="kqb-notes">
              {notes.map((n, i) => (
                <NoteEntry key={i} note={n} />
              ))}
            </ul>
          </div>
        );
      })}
    </>
  );
}

// ── list_tasks: a compact list of tasks with lane + priority + tags ────

export function ListTasksBody({ output, isRunning }: { output: string | null; isRunning: boolean }) {
  const data = useMemo(() => asObj(output), [output]);
  const tasks = asArr(data.tasks);
  if (!output) {
    return <div className="kanban-card-muted">{isRunning ? "Listing tasks…" : "No tasks"}</div>;
  }
  if (tasks.length === 0) return <div className="kanban-card-muted">No tasks found</div>;

  return (
    <ul className="kqb-task-list">
      {tasks.map((t, i) => {
        const task = asObj(t);
        const title = str(task.title) || shortId(str(task.id));
        const lane = str(task.lane);
        const priority = str(task.priority);
        const tags = asArr(task.tags).map(str).filter(Boolean);
        return (
          <li key={str(task.id) || i} className="kqb-task">
            <span className="kqb-task-title">{title}</span>
            <span className="kqb-task-meta">
              {lane && <span className="kanban-lane-pill kanban-lane-pill-current">{lane}</span>}
              {priority && (
                <span className={`kanban-prio kanban-prio-${priority.toLowerCase()}`}>
                  <Flag size={9} /> {priority}
                </span>
              )}
              {tags.map((tag) => (
                <span key={tag} className="kanban-tag">
                  <Tag size={9} /> {tag}
                </span>
              ))}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

// ── board_summary: lanes with their active/archived counts ─────────────

export function BoardSummaryBody({ output, isRunning }: { output: string | null; isRunning: boolean }) {
  const data = useMemo(() => asObj(output), [output]);
  const lanes = asArr(data.lanes);
  if (!output) {
    return <div className="kanban-card-muted">{isRunning ? "Loading board…" : "No board"}</div>;
  }
  const boardName = str(data.board_name);
  const totalActive = num(data.total_active);

  return (
    <>
      {(boardName || totalActive !== null) && (
        <div className="kqb-board-head">
          {boardName && <span className="kqb-board-name">{boardName}</span>}
          {totalActive !== null && (
            <span className="kqb-board-total">
              <Layers size={10} /> {totalActive} active
            </span>
          )}
        </div>
      )}
      <ul className="kqb-lane-list">
        {lanes.map((l, i) => {
          const lane = asObj(l);
          const active = num(lane.active_count) ?? 0;
          const archived = num(lane.archived_count) ?? 0;
          const wip = num(lane.wip);
          return (
            <li key={str(lane.id) || i} className="kqb-lane-row">
              <span className={`kqb-lane-name ${lane.terminal ? "kqb-lane-terminal" : ""}`}>
                {str(lane.name)}
              </span>
              <span className="kqb-lane-counts">
                <span className="kqb-lane-count" title="active">{active}{wip !== null ? `/${wip}` : ""}</span>
                {archived > 0 && <span className="kqb-lane-archived" title="archived">{archived} archived</span>}
              </span>
            </li>
          );
        })}
      </ul>
    </>
  );
}
