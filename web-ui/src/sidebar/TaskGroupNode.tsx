import React from "react";
import { SquareKanban, ChevronDown, ChevronRight } from "lucide-react";
import type { SessionInfo } from "../api";
import { formatTime } from "./formatTime";

/** A kanban task and the (root) sessions launched from it. */
export interface TaskGroup {
  taskId: string;
  taskTitle: string;
  /** Lane colour of the group's most-recent session, for the accent rail. */
  laneColor: string;
  /** Most recent `time.updated` across the group's sessions. */
  lastUpdated: number;
  sessions: SessionInfo[];
}

/** A kanban-task group in the sidebar: a collapsible, lane-tinted card whose
 *  header toggles its nested sessions and back-links to the originating task. */
export const TaskGroupNode = React.memo(function TaskGroupNode({
  group,
  isExpanded,
  onToggleExpand,
  onOpenKanbanTask,
  renderSession,
}: {
  group: TaskGroup;
  isExpanded: boolean;
  onToggleExpand: (taskId: string) => void;
  onOpenKanbanTask?: (taskId: string) => void;
  renderSession: (session: SessionInfo) => React.ReactNode;
}) {
  return (
    <div
      className={`sb-task-group${isExpanded ? " sb-task-group-open" : ""}`}
      style={
        group.laneColor
          ? ({ "--lane-color": group.laneColor } as React.CSSProperties)
          : undefined
      }
    >
      <div className="sb-task-group-header">
        <button
          type="button"
          className="sb-task-group-toggle"
          onClick={() => onToggleExpand(group.taskId)}
          aria-expanded={isExpanded}
          title={group.taskTitle}
        >
          <span className="sb-task-group-chevron">
            {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          </span>
          <SquareKanban size={12} className="sb-task-group-icon" />
          <span className="sb-task-group-title">{group.taskTitle}</span>
          <span className="sb-task-group-time">{formatTime(group.lastUpdated)}</span>
        </button>
        <button
          type="button"
          className="sb-task-group-count"
          onClick={onOpenKanbanTask ? () => onOpenKanbanTask(group.taskId) : undefined}
          title={`Open kanban task · ${group.taskTitle}`}
        >
          {group.sessions.length}
        </button>
      </div>
      {isExpanded && (
        <div className="sb-task-group-sessions">
          {group.sessions.map(renderSession)}
        </div>
      )}
    </div>
  );
});
