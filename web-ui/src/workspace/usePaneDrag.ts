import { useCallback, useMemo, useState } from "react";
import type { DropEdge } from "./move";
import type { WorkspaceAction } from "./reducer";
import { findPane } from "./tree";
import { withViewTransition } from "./viewTransition";
import type { Node, PaneId, WidgetState, WindowId } from "./types";

/**
 * Dragging a pane's widget onto another pane or window.
 *
 * Kept apart from targeting: the targeting commands (1-9, s/v, n) are gated on
 * it, and a pointer drag must not arm a keymap the user cannot see.
 */
export function usePaneDrag(
  dispatch: (action: WorkspaceAction) => void,
  root: Node,
  describe: (widget: WidgetState) => string,
) {
  const [dragSource, setDragSource] = useState<PaneId | null>(null);

  /**
   * A pane dropped on another pane. The edge decides which drop it is; the
   * reducer owns both, so this only has to end the gesture.
   *
   * Through a view transition, for the reason `WorkspaceRoot` gives for closing
   * a pane: an edge drop re-seats one pane and leaves its old siblings growing
   * into the space, and there is no element left to animate once React has
   * moved it.
   */
  const dropWidget = useCallback(
    (pane: PaneId, edge: DropEdge) => {
      if (dragSource) {
        withViewTransition(() =>
          dispatch({ type: "dropPane", pane: dragSource, target: pane, edge }),
        );
      }
      setDragSource(null);
    },
    [dispatch, dragSource],
  );

  /** The same drag, let go over another window — or over "new window". */
  const dropOnWindow = useCallback(
    (window: WindowId | "new") => {
      if (dragSource) dispatch({ type: "movePaneToWindow", pane: dragSource, window });
      setDragSource(null);
    },
    [dispatch, dragSource],
  );
  const endDrag = useCallback(() => setDragSource(null), []);

  const draggedLabel = useMemo(() => {
    if (!dragSource) return "";
    const pane = findPane(root, dragSource);
    return pane?.widget ? describe(pane.widget) : "";
  }, [describe, dragSource, root]);

  return { dragSource, setDragSource, endDrag, dropWidget, dropOnWindow, draggedLabel };
}
