import { useCallback, useEffect, useMemo, useState } from "react";
import type { Lane, Task } from "../api/kanban";

/**
 * Keyboard selection for the board.
 *
 * Two-dimensional, because a board is: `j`/`k` walk the cards in a lane and
 * `h`/`l` cross lanes. Crossing keeps the row position where it can — moving
 * from the third card of one lane to the third of the next — and clamps to the
 * last card when the next lane is shorter, which is what makes a sweep across a
 * ragged board feel continuous.
 *
 * Empty lanes are skipped when crossing: landing on one would leave the user
 * with a selection they cannot act on and no obvious way back.
 */

export interface BoardSelection {
  readonly selectedId: string | undefined;
  readonly selectedTask: Task | undefined;
  select: (taskId: string) => void;
  moveDown: () => void;
  moveUp: () => void;
  moveLeft: () => void;
  moveRight: () => void;
}

interface Position {
  readonly laneIndex: number;
  readonly cardIndex: number;
}

function columnsOf(lanes: readonly Lane[], byLane: ReadonlyMap<string, Task[]>): Task[][] {
  return lanes.map((lane) =>
    [...(byLane.get(lane.id) ?? [])].sort((a, b) => a.order_index - b.order_index),
  );
}

function locate(columns: readonly Task[][], taskId: string | undefined): Position | undefined {
  if (!taskId) return undefined;
  for (let laneIndex = 0; laneIndex < columns.length; laneIndex += 1) {
    const cardIndex = columns[laneIndex].findIndex((task) => task.id === taskId);
    if (cardIndex >= 0) return { laneIndex, cardIndex };
  }
  return undefined;
}

/** First non-empty lane at or beyond `from`, searching in `delta`'s direction. */
function nextPopulatedLane(
  columns: readonly Task[][],
  from: number,
  delta: number,
): number | undefined {
  for (let index = from; index >= 0 && index < columns.length; index += delta) {
    if (columns[index].length > 0) return index;
  }
  return undefined;
}

export function useBoardSelection(
  lanes: readonly Lane[],
  tasksByLane: ReadonlyMap<string, Task[]>,
): BoardSelection {
  const columns = useMemo(() => columnsOf(lanes, tasksByLane), [lanes, tasksByLane]);
  const [selectedId, setSelectedId] = useState<string>();

  const position = useMemo(() => locate(columns, selectedId), [columns, selectedId]);

  const selectedTask = useMemo(
    () => (position ? columns[position.laneIndex][position.cardIndex] : undefined),
    [columns, position],
  );

  /** Recover the selection when the board reloads or the task is archived. */
  useEffect(() => {
    if (selectedId && position) return;
    const lane = nextPopulatedLane(columns, 0, 1);
    setSelectedId(lane === undefined ? undefined : columns[lane][0].id);
  }, [columns, position, selectedId]);

  const moveWithinLane = useCallback(
    (delta: number) => {
      if (!position) return;
      const column = columns[position.laneIndex];
      const next = Math.min(Math.max(position.cardIndex + delta, 0), column.length - 1);
      setSelectedId(column[next]?.id);
    },
    [columns, position],
  );

  const moveAcrossLanes = useCallback(
    (delta: number) => {
      if (!position) return;
      const lane = nextPopulatedLane(columns, position.laneIndex + delta, delta);
      if (lane === undefined) return;
      const column = columns[lane];
      const cardIndex = Math.min(position.cardIndex, column.length - 1);
      setSelectedId(column[cardIndex]?.id);
    },
    [columns, position],
  );

  return {
    selectedId,
    selectedTask,
    select: useCallback((taskId: string) => setSelectedId(taskId), []),
    moveDown: useCallback(() => moveWithinLane(1), [moveWithinLane]),
    moveUp: useCallback(() => moveWithinLane(-1), [moveWithinLane]),
    moveLeft: useCallback(() => moveAcrossLanes(-1), [moveAcrossLanes]),
    moveRight: useCallback(() => moveAcrossLanes(1), [moveAcrossLanes]),
  };
}
