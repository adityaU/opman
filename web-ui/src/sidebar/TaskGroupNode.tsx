import React from "react";
import { SquareKanban } from "lucide-react";
import type { SessionInfo } from "../api";

/** A kanban task and the (root) sessions launched from it. */
export interface TaskGroup {
  taskId: string;
  taskTitle: string;
  /** Lane colour of the group's most-recent session, for the accent rail. */
  laneColor: string;
  sessions: SessionInfo[];
}

/** A kanban-task group in the sidebar: a lightweight lane-tinted section whose
 *  header back-links to the originating task, with its sessions nested below. */
export const TaskGroupNode = React.memo(function TaskGroupNode({
  group,
  onOpenKanbanTask,
  renderSession,
}: {
  group: TaskGroup;
  onOpenKanbanTask?: (taskId: string) => void;
  renderSession: (session: SessionInfo) => React.ReactNode;
}) {
  return (
    <div
      className="sb-task-group"
      style={
        group.laneColor
          ? ({ "--lane-color": group.laneColor } as React.CSSProperties)
          : undefined
      }
    >
      <button
        type="button"
        className="sb-task-group-header"
        onClick={onOpenKanbanTask ? () => onOpenKanbanTask(group.taskId) : undefined}
        title={`Kanban task · ${group.taskTitle}`}
      >
        <SquareKanban size={12} className="sb-task-group-icon" />
        <span className="sb-task-group-title">{group.taskTitle}</span>
        <span className="sb-task-group-count">{group.sessions.length}</span>
      </button>
      <div className="sb-task-group-sessions">
        {group.sessions.map(renderSession)}
      </div>
    </div>
  );
});
