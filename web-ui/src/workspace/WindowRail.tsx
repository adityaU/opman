import React, { useCallback } from "react";
import { PanelRightClose, Plus } from "lucide-react";
import { KeyHint } from "../keybindings/hint/KeyHint";
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
  readonly onToggle: () => void;
}

export const WindowRail: React.FC<WindowRailProps> = React.memo(function WindowRail({
  windows,
  activeWindowId,
  expanded,
  busyWindows,
  onActivate,
  onNewWindow,
  onRename,
  onToggle,
}) {
  // Roving focus: the rail is one tab stop and the arrows move within it, so
  // Tab order stays linear no matter how many windows are open.
  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
      if (step !== 0) {
        const index = windows.findIndex((w) => w.id === activeWindowId);
        const next = windows[(index + step + windows.length) % windows.length];
        onActivate(next.id);
        event.preventDefault();
        return;
      }
      if (event.key === "F2") {
        onRename(activeWindowId);
        event.preventDefault();
      }
    },
    [activeWindowId, onActivate, onRename, windows],
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
      >
        {windows.map((window, index) => {
          const active = window.id === activeWindowId;
          return (
            <button
              key={window.id}
              type="button"
              role="tab"
              // The rename field is portalled and finds its chip through this.
              data-window-id={window.id}
              aria-selected={active}
              tabIndex={active ? 0 : -1}
              className={
                "wsp-rail-chip" +
                (active ? " is-active" : "") +
                (busyWindows.has(window.id) ? " is-busy" : "")
              }
              onClick={() => onActivate(window.id)}
              onDoubleClick={() => onRename(window.id)}
              title={`${window.name} — window ${index + 1}`}
            >
              <span className="wsp-rail-chip-name">{window.name}</span>
              <span className="wsp-rail-chip-pulse" aria-hidden="true" />
            </button>
          );
        })}
      </div>

      <KeyHint label="New window" command="workspace.newWindow" placement="left">
        <button type="button" className="wsp-rail-add" onClick={onNewWindow} aria-label="New window">
          <Plus size={14} />
        </button>
      </KeyHint>
    </div>
  );
});
