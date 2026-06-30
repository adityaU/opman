import React, { useState, useCallback } from "react";
import { Archive } from "lucide-react";
import type { Task, Lane } from "../api/kanban";
import { TaskCard } from "./TaskCard";

interface Props {
  /** Archived tasks for this board. */
  tasks: Task[];
  /** Lane lookup for each card's accent color. */
  lanesById: Map<string, Lane>;
  /** True while any card is being dragged — keeps the column expanded as a drop target. */
  isDragging: boolean;
  /** Archive the currently-dragged task (called on drop). */
  onArchiveDrop: () => void;
  /** Begin dragging an archived card (marks it as an archive-sourced drag). */
  onCardDragStart: (taskId: string) => void;
  onDragEnd: () => void;
  onOpenDetail: (task: Task) => void;
  onLaunchTask: (task: Task) => void;
  onOpenSession: (sessionId: string) => void;
}

/**
 * A slim column pinned to the end of the board. Collapsed it shows just an icon +
 * count; on hover (or while a card is being dragged) it expands to a normal lane
 * width and full height, showing archived tasks as normal cards. Drop a task in to
 * archive it; drag a card out (to any lane) to restore it. Archiving is distinct from
 * deleting — the task is kept.
 */
export const ArchiveColumn: React.FC<Props> = function ArchiveColumn(p) {
  const [isOver, setIsOver] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (!isOver) setIsOver(true);
  }, [isOver]);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    if (e.currentTarget === e.target) setIsOver(false);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsOver(false);
    p.onArchiveDrop();
  }, [p]);

  const sorted = [...p.tasks].sort((a, b) => b.updated_at.localeCompare(a.updated_at));

  return (
    <div
      className={`kanban-lane kanban-archive-lane${p.isDragging ? " kanban-archive-armed" : ""}${
        isOver ? " kanban-lane-dropping" : ""
      }`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      title="Archive — drop a task here to archive it"
    >
      <div className="kanban-lane-header kanban-archive-header">
        <Archive size={13} className="kanban-archive-icon" />
        <span className="kanban-lane-name kanban-archive-name">Archive</span>
        <span className="kanban-lane-count">{p.tasks.length}</span>
      </div>

      <div className="kanban-lane-body kanban-archive-body">
        {sorted.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            lane={p.lanesById.get(task.lane_id)}
            onDragStart={p.onCardDragStart}
            onDragEnd={p.onDragEnd}
            onOpenDetail={p.onOpenDetail}
            onLaunch={p.onLaunchTask}
            onOpenSession={p.onOpenSession}
          />
        ))}
        {sorted.length === 0 && <div className="kanban-lane-empty">Drop tasks here to archive</div>}
      </div>
    </div>
  );
};
