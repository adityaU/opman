import React, { useCallback, useMemo, useState } from "react";
import { PanelRightClose, Plus, X } from "lucide-react";
import { KeyHint } from "../keybindings/hint/KeyHint";
import { CloseWindowConfirm } from "./CloseWindowConfirm";
import { useRailReorder } from "./useRailReorder";
import type { WindowId, WorkspaceWindow } from "./types";

/**
 * The window column on the right of the panes, and the spine it collapses to.
 *
 * A rail rather than a tab strip because width is the axis a wide monitor has
 * spare and height is the one it does not: a strip bills ~34px off every pane
 * in the window, a rail bills one chip's width once. It also scales — fifteen
 * chips stack fine where fifteen tabs would have to scroll.
 *
 * The width is the content's. Windows are named "1".."9" until someone renames
 * one, so the common case costs a single digit's width; a rail full of worded
 * names grows to fit them and stops at a ceiling (see workspace-1.css).
 *
 * Hidden, it leaves the spine rather than nothing. Losing the ability to see
 * that an agent is running in window 3 is too high a price for 30px, and one
 * dot per window costs effectively zero. The spine is itself the button that
 * brings the rail back, so hiding it is never a one-way door for the mouse.
 */

interface WindowRailProps {
  readonly windows: readonly WorkspaceWindow[];
  readonly activeWindowId: WindowId;
  readonly expanded: boolean;
  /** Window ids with at least one busy agent inside. */
  readonly busyWindows: ReadonlySet<WindowId>;
  readonly onActivate: (id: WindowId) => void;
  readonly onNewWindow: () => void;
  readonly onRename: (id: WindowId) => void;
  readonly onClose: (id: WindowId) => void;
  readonly onToggle: () => void;
  /** Drop `id` in front of `before`, or at the end of the rail for null. */
  readonly onReorder: (id: WindowId, before: WindowId | null) => void;
}

export const WindowRail: React.FC<WindowRailProps> = React.memo(function WindowRail({
  windows,
  activeWindowId,
  expanded,
  busyWindows,
  onActivate,
  onNewWindow,
  onRename,
  onClose,
  onToggle,
  onReorder,
}) {
  // The window whose × has been pressed, if any. Local because it is a
  // transient answer to a click on this control, not workspace state.
  const [confirming, setConfirming] = useState<WindowId | null>(null);

  // The last window has no × at all: closing it is a no-op in the reducer, and
  // a control that asks a question and then does nothing is worse than absent.
  const closable = windows.length > 1;

  const order = useMemo(() => windows.map((window) => window.id), [windows]);
  const drag = useRailReorder(order, onReorder);

  const confirm = useCallback(() => {
    if (confirming) onClose(confirming);
    setConfirming(null);
  }, [confirming, onClose]);
  // Roving focus: the rail is one tab stop and the arrows move within it, so
  // Tab order stays linear no matter how many windows are open.
  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
      if (step !== 0) {
        const index = windows.findIndex((w) => w.id === activeWindowId);
        event.preventDefault();
        // Alt turns the arrows from "which window" into "where this window
        // goes": the same reorder the drag performs, for a keyboard that cannot
        // drag. Ends of the rail hold rather than wrap — a move is a nudge, and
        // teleporting the chip to the far end is never what was meant.
        if (event.altKey) {
          const target = index + step;
          if (target < 0 || target >= windows.length) return;
          onReorder(activeWindowId, step === 1 ? windows[target + 1]?.id ?? null : windows[target].id);
          return;
        }
        const next = windows[(index + step + windows.length) % windows.length];
        onActivate(next.id);
        return;
      }
      if (event.key === "F2") {
        onRename(activeWindowId);
        event.preventDefault();
      }
    },
    [activeWindowId, onActivate, onRename, onReorder, windows],
  );

  if (!expanded) {
    return (
      <KeyHint label="Show windows" command="workspace.toggleRail" placement="left">
        <button type="button" className="wsp-spine" onClick={onToggle} aria-label="Show windows">
          {windows.map((window) => (
            <span
              key={window.id}
              className={
                "wsp-spine-dot" +
                (window.id === activeWindowId ? " is-active" : "") +
                (busyWindows.has(window.id) ? " is-busy" : "")
              }
            />
          ))}
        </button>
      </KeyHint>
    );
  }

  return (
    // The tablist role is on the list, not the rail: a tablist may only
    // contain tabs, and the rail also holds the collapse and new-window
    // buttons. The arrow-key handler stays out here so it catches them from
    // anywhere in the rail.
    <div className="wsp-rail" onKeyDown={onKeyDown}>
      <KeyHint label="Hide windows" command="workspace.toggleRail" placement="left">
        <button type="button" className="wsp-rail-toggle" onClick={onToggle} aria-label="Hide windows">
          <PanelRightClose size={14} />
        </button>
      </KeyHint>

      <div
        className="wsp-rail-list"
        role="tablist"
        aria-orientation="vertical"
        aria-label="Windows"
        onDrop={drag.onDrop}
      >
        {windows.map((window, index) => {
          const active = window.id === activeWindowId;
          const marker = drag.marker?.id === window.id ? drag.marker.side : undefined;
          return (
            // A div rather than a button: the chip carries its own close
            // control, and a button inside a button is not valid HTML. `role`
            // and the key handler give back everything the element loses.
            <div
              key={window.id}
              role="tab"
              // The rename field and the confirmation are portalled and find
              // their chip through this.
              data-window-id={window.id}
              aria-selected={active}
              tabIndex={active ? 0 : -1}
              // The whole chip is the handle. It has one job — be the window —
              // so a separate grip would only cost width the rail sizes itself
              // by, and dragging what you see is the plainer gesture.
              draggable
              data-drop={marker}
              className={
                "wsp-rail-chip" +
                (active ? " is-active" : "") +
                (busyWindows.has(window.id) ? " is-busy" : "") +
                (drag.dragging === window.id ? " is-dragging" : "")
              }
              onDragStart={(event) => drag.onDragStart(window.id, event)}
              onDragOver={(event) => drag.onDragOver(window.id, event)}
              onDragEnd={drag.onDragEnd}
              onClick={() => onActivate(window.id)}
              onDoubleClick={() => onRename(window.id)}
              onKeyDown={(event) => {
                if (event.key !== "Enter" && event.key !== " ") return;
                event.preventDefault();
                onActivate(window.id);
              }}
              title={`${window.name} — window ${index + 1}`}
            >
              <span className="wsp-rail-chip-name">{window.name}</span>
              <span className="wsp-rail-chip-pulse" aria-hidden="true" />
              {closable && (
                <button
                  type="button"
                  className="wsp-rail-chip-close"
                  tabIndex={active ? 0 : -1}
                  aria-label={`Close ${window.name}`}
                  onClick={(event) => {
                    // Otherwise the chip underneath also activates the window
                    // the user is in the middle of asking to close.
                    event.stopPropagation();
                    setConfirming(window.id);
                  }}
                >
                  <X size={11} />
                </button>
              )}
            </div>
          );
        })}
      </div>

      <KeyHint label="New window" command="workspace.newWindow" placement="left">
        <button type="button" className="wsp-rail-add" onClick={onNewWindow} aria-label="New window">
          <Plus size={14} />
        </button>
      </KeyHint>

      {confirming && (
        <CloseWindowConfirm
          windowId={confirming}
          name={windows.find((w) => w.id === confirming)?.name ?? ""}
          onConfirm={confirm}
          onCancel={() => setConfirming(null)}
        />
      )}
    </div>
  );
});
