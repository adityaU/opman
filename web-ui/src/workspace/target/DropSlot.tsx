import React, { useCallback, useState } from "react";
import { edgeFromPointer, type DropEdge } from "../move";
import type { PaneId } from "../types";
import type { TargetSlot } from "./useTargeting";

/**
 * One pane as a drop target, divided into five regions.
 *
 * The regions are read from the pointer rather than built as five child
 * elements. Five nested drop targets means five `dragover` streams, five
 * `dragleave` races on the way between them, and a shape that has to be kept
 * agreeing with the arithmetic anyway; one element that asks
 * `edgeFromPointer` where the pointer is has neither problem, and the preview
 * is a single pseudo-element the compositor can move on its own.
 *
 * The preview is the *result*, not the cursor: half the pane lit on the side
 * the pane would land, or the whole pane for a swap. Nothing else says which of
 * the two drops is about to happen.
 */

interface DropSlotProps {
  readonly slot: TargetSlot;
  readonly rect: { readonly top: number; readonly left: number; readonly width: number; readonly height: number };
  /** True for the pane the drag started in: it cannot receive itself. */
  readonly source: boolean;
  readonly onDrop: (pane: PaneId, edge: DropEdge) => void;
}

export const DropSlot: React.FC<DropSlotProps> = function DropSlot({ slot, rect, source, onDrop }) {
  const [edge, setEdge] = useState<DropEdge | null>(null);

  const onDragOver = useCallback(
    (event: React.DragEvent<HTMLButtonElement>) => {
      if (source) return;
      // preventDefault is what marks the element as a drop target at all.
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      const box = event.currentTarget.getBoundingClientRect();
      const next = edgeFromPointer(box, event.clientX, event.clientY);
      setEdge((current) => (current === next ? current : next));
    },
    [source],
  );

  const drop = useCallback(
    (event: React.DragEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();
      const box = event.currentTarget.getBoundingClientRect();
      setEdge(null);
      if (!source) onDrop(slot.paneId, edgeFromPointer(box, event.clientX, event.clientY));
    },
    [onDrop, slot.paneId, source],
  );

  return (
    <button
      type="button"
      className={
        "wsp-target-slot is-drop" +
        (slot.focused ? " is-focused" : "") +
        (source ? " is-source" : "") +
        (edge ? " is-over" : "")
      }
      style={{ top: rect.top, left: rect.left, width: rect.width, height: rect.height }}
      data-edge={edge ?? undefined}
      onDragOver={onDragOver}
      onDragLeave={() => setEdge(null)}
      onDrop={drop}
      aria-label={`Pane ${slot.ordinal}: drop on an edge to move here, in the middle to swap`}
    >
      <span className="wsp-target-number">{slot.ordinal}</span>
    </button>
  );
};
