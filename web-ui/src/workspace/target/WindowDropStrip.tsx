import React, { useCallback, useState } from "react";
import { Plus } from "lucide-react";
import type { WindowId } from "../types";

/**
 * The other windows, as drop targets, down the edge the rail lives on.
 *
 * The rail itself cannot serve: the drop overlay is a full-screen portal above
 * everything, so a chip underneath it never sees the pointer. Re-drawing the
 * windows inside the overlay also means the gesture works with the rail hidden,
 * and it puts the two answers to "where does this go" — a pane, or a window —
 * on the same surface instead of one over the tree and one behind it.
 */

export interface WindowDropTarget {
  readonly id: WindowId;
  readonly name: string;
}

interface WindowDropStripProps {
  readonly windows: readonly WindowDropTarget[];
  readonly onDrop: (target: WindowId | "new") => void;
}

export const WindowDropStrip: React.FC<WindowDropStripProps> = function WindowDropStrip({
  windows,
  onDrop,
}) {
  const [over, setOver] = useState<WindowId | "new" | null>(null);

  const enter = useCallback((target: WindowId | "new", event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    setOver((current) => (current === target ? current : target));
  }, []);

  const drop = useCallback(
    (target: WindowId | "new", event: React.DragEvent) => {
      event.preventDefault();
      event.stopPropagation();
      setOver(null);
      onDrop(target);
    },
    [onDrop],
  );

  return (
    <div className="wsp-target-windows" aria-label="Move to another window">
      {windows.map((window) => (
        <button
          key={window.id}
          type="button"
          className={"wsp-target-window" + (over === window.id ? " is-over" : "")}
          onDragOver={(event) => enter(window.id, event)}
          onDragLeave={() => setOver((current) => (current === window.id ? null : current))}
          onDrop={(event) => drop(window.id, event)}
          aria-label={`Move to window ${window.name}`}
        >
          <span className="wsp-target-window-name">{window.name}</span>
        </button>
      ))}
      <button
        type="button"
        className={"wsp-target-window is-new" + (over === "new" ? " is-over" : "")}
        onDragOver={(event) => enter("new", event)}
        onDragLeave={() => setOver((current) => (current === "new" ? null : current))}
        onDrop={(event) => drop("new", event)}
        aria-label="Move to a new window"
      >
        <Plus size={14} />
      </button>
    </div>
  );
};
