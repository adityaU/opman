import React, { useState, useCallback } from "react";
import type { Lane, Task } from "../api/kanban";
import { TaskCard } from "./TaskCard";

interface Props {
  lane: Lane;
  tasks: Task[];
  /** taskIds that currently have an attachment (set when detail known); optional. */
  attachmentTaskIds?: Set<string>;
  /** The lane the currently-dragged task came from, or null. */
  dragSourceLaneId: string | null;
  /** Whether dropping into this lane from the drag source is allowed. */
  canDropHere: boolean;
  onDragStart: (taskId: string, fromLaneId: string) => void;
  onDragEnd: () => void;
  /** Drop a task into this lane at the given index position. */
  onDrop: (laneId: string, beforeOrderIndex: number | null, afterOrderIndex: number | null) => void;
  onOpenDetail: (task: Task) => void;
  onLaunchTask: (task: Task) => void;
  onOpenSession: (sessionId: string) => void;
  selectedTaskId?: string;
  onSelectTask?: (taskId: string) => void;
}

export const KanbanLane: React.FC<Props> = function KanbanLane(p) {
  const { lane, tasks } = p;
  const [isOver, setIsOver] = useState(false);

  const sorted = [...tasks].sort((a, b) => a.order_index - b.order_index);
  const overLimit = lane.wip != null && tasks.length > lane.wip;

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      if (!p.canDropHere) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (!isOver) setIsOver(true);
    },
    [p.canDropHere, isOver],
  );

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    // Only clear when leaving the lane element itself (not entering a child).
    if (e.currentTarget === e.target) setIsOver(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsOver(false);
      if (!p.canDropHere) return;
      // Drop at the end of the lane by default.
      const last = sorted.length ? sorted[sorted.length - 1].order_index : null;
      p.onDrop(lane.id, last, null);
    },
    [p, sorted, lane.id],
  );

  return (
    <div
      className={`kanban-lane${isOver && p.canDropHere ? " kanban-lane-dropping" : ""}${
        p.dragSourceLaneId && !p.canDropHere ? " kanban-lane-blocked" : ""
      }`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      <div className="kanban-lane-header" style={{ borderTop: `3px solid ${lane.color}` }}>
        <span className="kanban-lane-accent" style={{ background: lane.color }} />
        <span className="kanban-lane-name">{lane.name}</span>
        {lane.terminal && <span className="kanban-lane-terminal" title="Terminal review lane">★</span>}
        <span className={`kanban-lane-count${overLimit ? " kanban-lane-count-over" : ""}`}>
          {tasks.length}
          {lane.wip != null ? ` / ${lane.wip}` : ""}
        </span>
      </div>

      <div className="kanban-lane-body">
        {sorted.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            lane={lane}
            onDragStart={(taskId) => p.onDragStart(taskId, lane.id)}
            onDragEnd={p.onDragEnd}
            onOpenDetail={p.onOpenDetail}
            selected={p.selectedTaskId === task.id}
            onSelect={p.onSelectTask}
            onLaunch={p.onLaunchTask}
            onOpenSession={p.onOpenSession}
            hasAttachment={p.attachmentTaskIds?.has(task.id)}
          />
        ))}
        {sorted.length === 0 && <div className="kanban-lane-empty">No tasks</div>}
      </div>
    </div>
  );
};
