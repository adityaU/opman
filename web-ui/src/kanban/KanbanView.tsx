import React, { useState, useEffect, useCallback, useMemo } from "react";
import { Settings2, Plus, LayoutGrid, RefreshCw } from "lucide-react";
import { useKanbanBoard, midpointOrder } from "./useKanbanBoard";
import { KanbanLane } from "./KanbanLane";
import { ArchiveColumn } from "./ArchiveColumn";
import { TaskEditorModal } from "./TaskEditorModal";
import { TaskDetailModal } from "./TaskDetailModal";
import { LaneConfigModal } from "./LaneConfigModal";
import { LaunchModal } from "./LaunchModal";
import { fetchAgents, type AgentInfo } from "../api/session";
import type { Board, Task, Lane } from "../api/kanban";
import type { ProjectInfo } from "../api/state";

/** Sentinel `fromLaneId` marking a drag that originated in the archive column. */
const ARCHIVE_SOURCE = "__archive__";

interface Props {
  /** All projects — the board has its own selector so it's self-contained. */
  projects: ProjectInfo[];
  /** Project to show, driven by `?project` in the URL. */
  projectIndex: number;
  /** Switch the board's project — persists to the URL via the route hook. */
  onSelectProject: (projectIndex: number) => void;
  /** Deep-link into a session's chat within the selected project. */
  onOpenSession: (sessionId: string, projectIndex: number) => void;
  onError: (msg: string) => void;
  /** When set, open this task's editor once the board has loaded (back-link from the sidebar). */
  focusTaskId?: string | null;
  /** Called after `focusTaskId` has been opened, so the URL param can be cleared. */
  onFocusTaskConsumed?: () => void;
}

interface DragState {
  taskId: string;
  fromLaneId: string;
}

export const KanbanView: React.FC<Props> = function KanbanView(p) {
  // The shown project is URL-driven (`?project`); switching it rewrites the URL.
  const selectedIndex = p.projectIndex;
  const project = useMemo(
    () => p.projects.find((pr) => pr.index === selectedIndex) ?? p.projects[selectedIndex],
    [p.projects, selectedIndex],
  );
  const board = useKanbanBoard(selectedIndex, project?.path, p.onError);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [drag, setDrag] = useState<DragState | null>(null);

  // Modal state
  const [editorTask, setEditorTask] = useState<Task | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorDefaultLane, setEditorDefaultLane] = useState<string | undefined>(undefined);
  const [configOpen, setConfigOpen] = useState(false);
  const [launchTask, setLaunchTask] = useState<Task | null>(null);
  const [detailTaskId, setDetailTaskId] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    fetchAgents().then((a) => {
      if (alive) setAgents(a.filter((x) => !x.hidden));
    });
    return () => {
      alive = false;
    };
  }, []);

  const lanesById = useMemo(() => {
    const map = new Map<string, Lane>();
    board.board?.lanes.forEach((l) => map.set(l.id, l));
    return map;
  }, [board.board]);

  const tasksByLane = useMemo(() => {
    const map = new Map<string, Task[]>();
    for (const t of board.tasks) {
      if (t.archived) continue; // archived tasks live in the archive column, not their lane
      const arr = map.get(t.lane_id) ?? [];
      arr.push(t);
      map.set(t.lane_id, arr);
    }
    return map;
  }, [board.tasks]);

  const archivedTasks = useMemo(
    () => board.tasks.filter((t) => t.archived),
    [board.tasks],
  );

  const handleDragStart = useCallback((taskId: string, fromLaneId: string) => {
    setDrag({ taskId, fromLaneId });
  }, []);

  const handleDragEnd = useCallback(() => setDrag(null), []);

  // Dropping onto the archive column archives the dragged task. (A card already
  // dragged FROM the archive that's dropped back here is a no-op.)
  const handleArchiveDrop = useCallback(() => {
    if (!drag) return;
    board.archiveTask(drag.taskId, true);
    setDrag(null);
  }, [drag, board]);

  const handleDrop = useCallback(
    (laneId: string, beforeOrderIndex: number | null, afterOrderIndex: number | null) => {
      if (!drag) return;
      // A card dragged out of the archive onto any lane is restored (unarchived) to its
      // original lane — the drop target lane is ignored to avoid transition conflicts.
      if (drag.fromLaneId === ARCHIVE_SOURCE) {
        board.archiveTask(drag.taskId, false);
        setDrag(null);
        return;
      }
      const order = midpointOrder(beforeOrderIndex, afterOrderIndex);
      board.moveTask(drag.taskId, laneId, order);
      setDrag(null);
    },
    [drag, board],
  );

  const openNewTask = useCallback(() => {
    setEditorTask(null);
    setEditorDefaultLane(board.board?.lanes[0]?.id);
    setEditorOpen(true);
  }, [board.board]);

  const openEditTask = useCallback((task: Task) => {
    setEditorTask(task);
    setEditorDefaultLane(undefined);
    setEditorOpen(true);
  }, []);

  const openDetail = useCallback((task: Task) => setDetailTaskId(task.id), []);

  // Back-link from a session: open the originating task's editor once its board
  // has loaded, then clear the URL param so it only fires once.
  const { focusTaskId, onFocusTaskConsumed } = p;
  useEffect(() => {
    if (!focusTaskId || !board.board) return;
    const task = board.tasks.find((t) => t.id === focusTaskId);
    if (!task) return;
    openEditTask(task);
    onFocusTaskConsumed?.();
  }, [focusTaskId, board.board, board.tasks, openEditTask, onFocusTaskConsumed]);

  const onOpenSession = useCallback(
    (sessionId: string) => p.onOpenSession(sessionId, selectedIndex),
    [p, selectedIndex],
  );

  if (board.loading && !board.board) {
    return (
      <div className="kanban-view kanban-view-loading">
        <div className="chat-loading-spinner" />
        <span>Loading board…</span>
      </div>
    );
  }

  if (board.error && !board.board) {
    return (
      <div className="kanban-view kanban-view-error">
        <p>Couldn't load the board: {board.error}</p>
        <button className="kanban-btn" onClick={board.refetch}>
          <RefreshCw size={13} /> Retry
        </button>
      </div>
    );
  }

  const b: Board | null = board.board;

  return (
    <div className="kanban-view">
      <div className="kanban-header">
        <div className="kanban-header-left">
          <LayoutGrid size={16} />
          {p.projects.length > 1 ? (
            <select
              className="kanban-project-select"
              value={selectedIndex}
              onChange={(e) => p.onSelectProject(Number(e.target.value))}
              title="Switch project board"
            >
              {p.projects.map((pr) => (
                <option key={pr.index} value={pr.index}>
                  {pr.name}
                </option>
              ))}
            </select>
          ) : (
            <h2 className="kanban-board-name">{project?.name || b?.name || "Board"}</h2>
          )}
        </div>
        <div className="kanban-header-actions">
          <button className="kanban-btn" onClick={() => setConfigOpen(true)} disabled={!b}>
            <Settings2 size={13} /> Configure lanes
          </button>
          <button className="kanban-btn kanban-btn-primary" onClick={openNewTask} disabled={!b}>
            <Plus size={13} /> New task
          </button>
        </div>
      </div>

      <div className="kanban-board-scroll">
        {b?.lanes.map((lane) => {
          // Archive-sourced drags can be restored into any lane; normal drags follow
          // the transition graph.
          const canDropHere = drag
            ? drag.fromLaneId === ARCHIVE_SOURCE || board.canMove(drag.fromLaneId, lane.id)
            : false;
          return (
            <KanbanLane
              key={lane.id}
              lane={lane}
              tasks={tasksByLane.get(lane.id) ?? []}
              dragSourceLaneId={drag?.fromLaneId ?? null}
              canDropHere={canDropHere}
              onDragStart={handleDragStart}
              onDragEnd={handleDragEnd}
              onDrop={handleDrop}
              onOpenDetail={openDetail}
              onLaunchTask={setLaunchTask}
              onOpenSession={onOpenSession}
            />
          );
        })}
        {b && (
          <ArchiveColumn
            tasks={archivedTasks}
            lanesById={lanesById}
            isDragging={!!drag}
            onArchiveDrop={handleArchiveDrop}
            onCardDragStart={(taskId) => handleDragStart(taskId, ARCHIVE_SOURCE)}
            onDragEnd={handleDragEnd}
            onOpenDetail={openDetail}
            onLaunchTask={setLaunchTask}
            onOpenSession={onOpenSession}
          />
        )}
      </div>

      {editorOpen && b && (
        <TaskEditorModal
          board={b}
          task={editorTask}
          defaultLaneId={editorDefaultLane}
          onClose={() => setEditorOpen(false)}
          onSaved={board.upsertTask}
          onDeleted={board.removeTask}
          onError={p.onError}
          onOpenSession={onOpenSession}
        />
      )}

      {configOpen && b && (
        <LaneConfigModal
          board={b}
          agents={agents}
          onClose={() => setConfigOpen(false)}
          onSaved={() => {
            setConfigOpen(false);
            board.refetch();
          }}
          onError={p.onError}
        />
      )}

      {launchTask && b && (
        <LaunchModal
          task={launchTask}
          board={b}
          lane={lanesById.get(launchTask.lane_id)}
          onClose={() => setLaunchTask(null)}
          onLaunched={(sessionId) => {
            setLaunchTask(null);
            board.refetch();
            p.onOpenSession(sessionId, selectedIndex);
          }}
          onError={p.onError}
        />
      )}

      {detailTaskId && b && (
        <TaskDetailModal
          board={b}
          taskId={detailTaskId}
          pipeline={board.pipelines.find((pl) => pl.task_id === detailTaskId)}
          onClose={() => setDetailTaskId(null)}
          onEdit={(task) => {
            setDetailTaskId(null);
            openEditTask(task);
          }}
          onLaunch={(task) => {
            setDetailTaskId(null);
            setLaunchTask(task);
          }}
          onOpenSession={onOpenSession}
          onError={p.onError}
        />
      )}
    </div>
  );
};
