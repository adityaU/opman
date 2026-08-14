import { useCallback, useState } from "react";
import type { WindowId } from "./types";

/**
 * Dragging window chips into a new order.
 *
 * The drag state is local to the rail because it is a gesture, not layout: only
 * its result — one `reorderWindow` — reaches the reducer, so a drag that ends
 * where it started leaves the workspace untouched and nothing re-renders.
 *
 * The drop target is an anchor chip plus a side rather than an index. Half a
 * chip is the smallest thing the pointer can be unambiguously inside of, and an
 * anchor stays correct while the list under the pointer is still the old order.
 */

type Side = "before" | "after";

export interface RailMarker {
  readonly id: WindowId;
  readonly side: Side;
}

export interface RailReorder {
  /** The chip being dragged, for the lifted-card styling. */
  readonly dragging: WindowId | null;
  /** Where the insertion line is drawn, if a drag is over the list. */
  readonly marker: RailMarker | null;
  readonly onDragStart: (id: WindowId, event: React.DragEvent<HTMLElement>) => void;
  readonly onDragOver: (id: WindowId, event: React.DragEvent<HTMLElement>) => void;
  readonly onDrop: (event: React.DragEvent<HTMLElement>) => void;
  readonly onDragEnd: () => void;
}

export function useRailReorder(
  order: readonly WindowId[],
  onReorder: (window: WindowId, before: WindowId | null) => void,
): RailReorder {
  const [dragging, setDragging] = useState<WindowId | null>(null);
  const [marker, setMarker] = useState<RailMarker | null>(null);

  const onDragStart = useCallback((id: WindowId, event: React.DragEvent<HTMLElement>) => {
    // Firefox refuses to start a drag with an empty payload, and the id is the
    // one thing worth putting in it.
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", id);
    setDragging(id);
  }, []);

  const onDragOver = useCallback(
    (id: WindowId, event: React.DragEvent<HTMLElement>) => {
      if (!dragging) return;
      // Without both of these the browser shows the no-drop cursor and never
      // fires `drop`.
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";

      const box = event.currentTarget.getBoundingClientRect();
      const side: Side = event.clientY < box.top + box.height / 2 ? "before" : "after";
      setMarker((previous) =>
        previous && previous.id === id && previous.side === side ? previous : { id, side },
      );
    },
    [dragging],
  );

  const onDragEnd = useCallback(() => {
    setDragging(null);
    setMarker(null);
  }, []);

  const onDrop = useCallback(
    (event: React.DragEvent<HTMLElement>) => {
      event.preventDefault();
      if (dragging && marker) {
        const index = order.indexOf(marker.id);
        const before = marker.side === "before" ? marker.id : order[index + 1] ?? null;
        onReorder(dragging, before);
      }
      onDragEnd();
    },
    [dragging, marker, onDragEnd, onReorder, order],
  );

  return { dragging, marker, onDragStart, onDragOver, onDrop, onDragEnd };
}
