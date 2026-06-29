import React, { useCallback } from "react";
import { Play, Paperclip, MessageSquare, Loader2 } from "lucide-react";
import type { Task, Lane, Priority, RunState } from "../api/kanban";

interface Props {
  task: Task;
  lane: Lane | undefined;
  onDragStart: (taskId: string) => void;
  onDragEnd: () => void;
  /** Open the task's detail modal (notes, stages, attachments). */
  onOpenDetail: (task: Task) => void;
  onLaunch: (task: Task) => void;
  /** Deep-link into the launched session's chat. */
  onOpenSession: (sessionId: string) => void;
  hasAttachment?: boolean;
}

const PRIORITY_LABEL: Record<Priority, string> = {
  low: "Low",
  normal: "Normal",
  high: "High",
  urgent: "Urgent",
};

const RUN_STATE_LABEL: Record<RunState, string> = {
  idle: "Idle",
  launching: "Launching",
  running: "Running",
  done: "Done",
  failed: "Failed",
};

export const TaskCard: React.FC<Props> = React.memo(function TaskCard(p) {
  const { task, lane } = p;

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.setData("text/plain", task.id);
      e.dataTransfer.effectAllowed = "move";
      p.onDragStart(task.id);
    },
    [task.id, p],
  );

  const isBusy = task.run_state === "launching" || task.run_state === "running";
  const accent = lane?.color || "var(--color-primary)";

  return (
    <div
      className="kanban-card liquid-glass"
      draggable
      onDragStart={handleDragStart}
      onDragEnd={p.onDragEnd}
      onClick={() => p.onOpenDetail(task)}
      style={{ borderLeft: `3px solid ${accent}` }}
    >
      <div className="kanban-card-title">{task.title || "Untitled"}</div>

      <div className="kanban-card-meta">
        <span className={`kanban-priority-chip kanban-priority-${task.priority}`}>
          {PRIORITY_LABEL[task.priority]}
        </span>
        {task.tags.map((tag) => (
          <span key={tag} className="kanban-tag-chip">
            {tag}
          </span>
        ))}
      </div>

      <div className="kanban-card-footer">
        <div className="kanban-card-indicators">
          {p.hasAttachment && (
            <span className="kanban-indicator" title="Has attachments">
              <Paperclip size={12} />
            </span>
          )}
          {task.run_state !== "idle" && (
            <button
              className={`kanban-runstate kanban-runstate-${task.run_state}`}
              title={
                task.session_id
                  ? `Open session (${RUN_STATE_LABEL[task.run_state]})`
                  : RUN_STATE_LABEL[task.run_state]
              }
              disabled={!task.session_id}
              onClick={(e) => {
                e.stopPropagation();
                if (task.session_id) p.onOpenSession(task.session_id);
              }}
            >
              {isBusy ? <Loader2 size={11} className="spinning" /> : <MessageSquare size={11} />}
              <span>{RUN_STATE_LABEL[task.run_state]}</span>
            </button>
          )}
        </div>

        {task.run_state === "idle" && (
          <button
            className="kanban-launch-btn"
            title="Launch agent for this task"
            onClick={(e) => {
              e.stopPropagation();
              p.onLaunch(task);
            }}
          >
            <Play size={11} />
            <span>Launch</span>
          </button>
        )}
      </div>
    </div>
  );
});
