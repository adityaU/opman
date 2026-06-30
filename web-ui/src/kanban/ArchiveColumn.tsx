import React, { useState, useCallback } from "react";
import { Archive, RotateCcw } from "lucide-react";
import type { Task } from "../api/kanban";

interface Props {
  /** Archived tasks for this board. */
  tasks: Task[];
  /** True while any card is being dragged — keeps the column expanded as a drop target. */
  isDragging: boolean;
  /** Archive the currently-dragged task (called on drop). */
  onArchiveDrop: () => void;
  /** Restore an archived task to its lane. */
  onUnarchive: (taskId: string) => void;
  onOpenDetail: (task: Task) => void;
}

/**
 * A slim column pinned to the end of the board. Collapsed it shows just an icon +
 * count; on hover (or while a card is being dragged) it expands to a normal lane
 * width so tasks can be dropped in to archive them. Archived tasks live here and
 * can be restored. Archiving is distinct from deleting — the task is kept.
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
          <div key={task.id} className="kanban-card kanban-archive-card">
            <button
              className="kanban-archive-card-title"
              onClick={() => p.onOpenDetail(task)}
              title={task.title}
              type="button"
            >
              {task.title}
            </button>
            <button
              className="kanban-archive-restore"
              onClick={() => p.onUnarchive(task.id)}
              title="Restore to its lane"
              type="button"
            >
              <RotateCcw size={12} /> Restore
            </button>
          </div>
        ))}
        {sorted.length === 0 && <div className="kanban-lane-empty">Drop tasks here to archive</div>}
      </div>
    </div>
  );
};
