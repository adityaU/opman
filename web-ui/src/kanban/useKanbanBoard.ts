import { useState, useEffect, useCallback, useRef } from "react";
import {
  fetchBoard,
  patchTask as apiPatchTask,
  type Board,
  type Task,
  type Transitions,
} from "../api/kanban";

export interface KanbanBoardState {
  board: Board | null;
  tasks: Task[];
  loading: boolean;
  error: string | null;
  /** Force a refetch. */
  refetch: () => void;
  /** Optimistically move a task to a new lane (and optional order). Reverts on failure. */
  moveTask: (taskId: string, laneId: string, orderIndex: number) => Promise<void>;
  /** Local mutation helper — replace/insert a task in state (e.g. after create/edit). */
  upsertTask: (task: Task) => void;
  /** Remove a task from local state (after delete). */
  removeTask: (taskId: string) => void;
  /** Whether a lane move is allowed by the transition graph. */
  canMove: (fromLaneId: string, toLaneId: string) => boolean;
}

/** Compute a fractional order_index as the midpoint between two neighbours.
 *  `before`/`after` are the order_index of the cards bracketing the drop slot. */
export function midpointOrder(before: number | null, after: number | null): number {
  if (before == null && after == null) return 1;
  if (before == null) return (after as number) - 1;
  if (after == null) return before + 1;
  return (before + after) / 2;
}

export function isAllowedTransition(
  transitions: Transitions,
  fromLaneId: string,
  toLaneId: string,
): boolean {
  if (fromLaneId === toLaneId) return true; // pure reorder within a lane
  const targets = transitions[fromLaneId];
  return Array.isArray(targets) && targets.includes(toLaneId);
}

export function useKanbanBoard(
  projectIndex: number,
  projectPath: string | undefined,
  onError?: (msg: string) => void,
): KanbanBoardState {
  const [board, setBoard] = useState<Board | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  const load = useCallback(async () => {
    try {
      const resp = await fetchBoard(projectIndex);
      if (!mountedRef.current) return;
      setBoard(resp.board);
      setTasks(resp.tasks);
      setError(null);
    } catch (e) {
      if (!mountedRef.current) return;
      setError(e instanceof Error ? e.message : "Failed to load board");
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, [projectIndex]);

  // Initial load + reload when the project changes.
  useEffect(() => {
    mountedRef.current = true;
    setLoading(true);
    load();
    return () => { mountedRef.current = false; };
  }, [load]);

  // ── SSE: refetch (debounced) on kanban_task / kanban_board for this project ──
  const debounceRef = useRef<number | null>(null);
  const projectPathRef = useRef(projectPath);
  projectPathRef.current = projectPath;

  useEffect(() => {
    const es = new EventSource("/api/events", { withCredentials: true });

    const scheduleRefetch = (eventProjectPath: string | undefined) => {
      // Only refetch if the event targets the active project (match by path when known).
      if (
        eventProjectPath &&
        projectPathRef.current &&
        eventProjectPath !== projectPathRef.current
      ) {
        return;
      }
      if (debounceRef.current != null) window.clearTimeout(debounceRef.current);
      debounceRef.current = window.setTimeout(() => { load(); }, 250);
    };

    const parsePath = (raw: string): string | undefined => {
      try {
        const data = JSON.parse(raw) as { project_path?: string };
        return data.project_path;
      } catch {
        return undefined;
      }
    };

    const onTask = (e: MessageEvent) => scheduleRefetch(parsePath(e.data));
    const onBoard = (e: MessageEvent) => scheduleRefetch(parsePath(e.data));

    es.addEventListener("kanban_task", onTask as EventListener);
    es.addEventListener("kanban_board", onBoard as EventListener);

    return () => {
      es.removeEventListener("kanban_task", onTask as EventListener);
      es.removeEventListener("kanban_board", onBoard as EventListener);
      es.close();
      if (debounceRef.current != null) window.clearTimeout(debounceRef.current);
    };
  }, [load]);

  const canMove = useCallback(
    (fromLaneId: string, toLaneId: string) =>
      board ? isAllowedTransition(board.transitions, fromLaneId, toLaneId) : false,
    [board],
  );

  const moveTask = useCallback(
    async (taskId: string, laneId: string, orderIndex: number) => {
      let previous: Task | undefined;
      setTasks((prev) =>
        prev.map((t) => {
          if (t.id !== taskId) return t;
          previous = t;
          return { ...t, lane_id: laneId, order_index: orderIndex };
        }),
      );
      try {
        const updated = await apiPatchTask(taskId, { lane_id: laneId, order_index: orderIndex });
        if (!mountedRef.current) return;
        setTasks((prev) => prev.map((t) => (t.id === taskId ? updated : t)));
      } catch (e) {
        // Revert optimistic move.
        if (mountedRef.current && previous) {
          const prevTask = previous;
          setTasks((prev) => prev.map((t) => (t.id === taskId ? prevTask : t)));
        }
        const msg =
          e instanceof Error && /409|transition/i.test(e.message)
            ? "That move isn't an allowed transition."
            : e instanceof Error
              ? e.message
              : "Failed to move task";
        onErrorRef.current?.(msg);
      }
    },
    [],
  );

  const upsertTask = useCallback((task: Task) => {
    setTasks((prev) => {
      const exists = prev.some((t) => t.id === task.id);
      return exists ? prev.map((t) => (t.id === task.id ? task : t)) : [...prev, task];
    });
  }, []);

  const removeTask = useCallback((taskId: string) => {
    setTasks((prev) => prev.filter((t) => t.id !== taskId));
  }, []);

  return { board, tasks, loading, error, refetch: load, moveTask, upsertTask, removeTask, canMove };
}
