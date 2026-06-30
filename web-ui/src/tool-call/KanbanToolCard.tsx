import React, { useMemo } from "react";
import {
  Trello,
  StickyNote,
  ArrowRightLeft,
  CheckCircle2,
  Tag,
  Flag,
  Loader2,
  AlertTriangle,
} from "lucide-react";
import { MessagePart } from "../types";

// ── Kanban MCP tool cards ─────────────────────────────────────────────
//
// Renders the `kanban_*` MCP tools (get_task / add_note / set_lane / complete)
// as purpose-built cards instead of the generic input/output accordion. Each
// action gets its own accent colour, icon and layout. Theme-aware: every colour
// resolves from CSS custom properties so glassy and flat both work.

type KanbanAction = "get_task" | "add_note" | "set_lane" | "complete" | "other";

/** Strip provider prefixes (mcp__kanban__kanban_get_task, kanban_kanban_get_task…) */
function kanbanAction(toolName: string): KanbanAction {
  const n = toolName.toLowerCase();
  if (n.includes("get_task")) return "get_task";
  if (n.includes("add_note")) return "add_note";
  if (n.includes("set_lane")) return "set_lane";
  if (n.includes("complete")) return "complete";
  return "other";
}

export function isKanbanTool(toolName: string): boolean {
  return toolName.toLowerCase().includes("kanban");
}

const asObj = (v: unknown): Record<string, unknown> => {
  if (v && typeof v === "object" && !Array.isArray(v)) return v as Record<string, unknown>;
  if (typeof v === "string") {
    try {
      const p = JSON.parse(v);
      if (p && typeof p === "object" && !Array.isArray(p)) return p as Record<string, unknown>;
    } catch {
      /* not JSON */
    }
  }
  return {};
};

const str = (v: unknown): string => (typeof v === "string" ? v : "");

const shortId = (id: string): string => {
  if (!id) return "";
  const tail = id.startsWith("tsk_") ? id.slice(4) : id;
  return tail.length > 8 ? `${tail.slice(0, 8)}…` : tail;
};

interface Lane {
  id: string;
  name: string;
  terminal?: boolean;
}

const ACTION_META: Record<KanbanAction, { label: string; Icon: typeof Trello; tone: string }> = {
  get_task: { label: "Kanban · Task", Icon: Trello, tone: "info" },
  add_note: { label: "Kanban · Note", Icon: StickyNote, tone: "note" },
  set_lane: { label: "Kanban · Move", Icon: ArrowRightLeft, tone: "move" },
  complete: { label: "Kanban · Complete", Icon: CheckCircle2, tone: "done" },
  other: { label: "Kanban", Icon: Trello, tone: "info" },
};

export function KanbanToolCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "";
  const action = kanbanAction(toolName);
  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";

  const input = useMemo(() => asObj(state?.input), [state?.input]);
  const output = useMemo(() => {
    const o = state?.output;
    return typeof o === "string" && o.length > 0 ? o : null;
  }, [state?.output]);

  const meta = ACTION_META[action];
  const taskId = str(input.task_id);

  return (
    <div className={`kanban-card kanban-card-${meta.tone} ${isError ? "kanban-card-error" : ""}`}>
      <div className="kanban-card-head">
        <span className="kanban-card-icon">
          <meta.Icon size={13} />
        </span>
        <span className="kanban-card-label">{meta.label}</span>
        {taskId && <span className="kanban-card-id">#{shortId(taskId)}</span>}
        <span className="kanban-card-status">
          {isError ? (
            <AlertTriangle size={12} className="tool-error-icon" />
          ) : isRunning ? (
            <Loader2 size={12} className="tool-spin-icon" />
          ) : (
            <CheckCircle2 size={12} className="tool-success-icon" />
          )}
        </span>
      </div>

      <div className="kanban-card-body">
        {action === "get_task" && <GetTaskBody output={output} isRunning={isRunning} />}
        {action === "add_note" && <AddNoteBody body={str(input.body)} />}
        {action === "set_lane" && <SetLaneBody lane={str(input.lane)} />}
        {action === "complete" && <CompleteBody summary={str(input.summary)} />}
        {action === "other" && output && <pre className="kanban-card-pre">{output}</pre>}

        {isError && (
          <div className="kanban-card-errmsg">
            <AlertTriangle size={12} />
            <span>{state?.error || "Tool call failed"}</span>
          </div>
        )}
      </div>
    </div>
  );
}

// ── get_task: title + lane pipeline + priority + tags ──────────────────

function GetTaskBody({ output, isRunning }: { output: string | null; isRunning: boolean }) {
  const task = useMemo(() => asObj(output), [output]);

  if (!output) {
    return (
      <div className="kanban-card-muted">
        {isRunning ? "Fetching task…" : "No task data"}
      </div>
    );
  }

  const title = str(task.title);
  const description = str(task.description);
  const priority = str(task.priority);
  const tags = Array.isArray(task.tags) ? (task.tags as unknown[]).map(str).filter(Boolean) : [];
  const lanes = Array.isArray(task.lanes) ? (task.lanes as Lane[]) : [];
  const current = asObj(task.current_lane);
  const currentId = str(current.id);

  return (
    <>
      {title && <div className="kanban-task-title">{title}</div>}
      {description && <div className="kanban-task-desc">{description}</div>}

      {lanes.length > 0 && (
        <div className="kanban-lane-track">
          {lanes.map((lane) => {
            const isCur = lane.id === currentId;
            return (
              <span
                key={lane.id}
                className={`kanban-lane-pill ${isCur ? "kanban-lane-pill-current" : ""} ${
                  lane.terminal ? "kanban-lane-pill-terminal" : ""
                }`}
                title={lane.name}
              >
                {lane.name}
              </span>
            );
          })}
        </div>
      )}

      {(priority || tags.length > 0) && (
        <div className="kanban-task-meta">
          {priority && (
            <span className={`kanban-prio kanban-prio-${priority.toLowerCase()}`}>
              <Flag size={10} /> {priority}
            </span>
          )}
          {tags.map((t) => (
            <span key={t} className="kanban-tag">
              <Tag size={10} /> {t}
            </span>
          ))}
        </div>
      )}
    </>
  );
}

// ── add_note: the note body styled like a sticky note ──────────────────

function AddNoteBody({ body }: { body: string }) {
  if (!body) return <div className="kanban-card-muted">Empty note</div>;
  return <div className="kanban-note">{body}</div>;
}

// ── set_lane: the destination lane as a move chip ──────────────────────

function SetLaneBody({ lane }: { lane: string }) {
  if (!lane) return <div className="kanban-card-muted">No lane specified</div>;
  return (
    <div className="kanban-move">
      <span className="kanban-move-arrow">
        <ArrowRightLeft size={12} />
      </span>
      <span className="kanban-move-label">Moving to</span>
      <span className="kanban-lane-pill kanban-lane-pill-current">{lane}</span>
    </div>
  );
}

// ── complete: the summary with a done banner ───────────────────────────

function CompleteBody({ summary }: { summary: string }) {
  return (
    <div className="kanban-complete">
      <div className="kanban-complete-banner">
        <CheckCircle2 size={13} />
        <span>Marked ready for review</span>
      </div>
      {summary && <div className="kanban-complete-summary">{summary}</div>}
    </div>
  );
}
