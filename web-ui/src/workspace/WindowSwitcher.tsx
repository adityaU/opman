import React, { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { AppWindow } from "lucide-react";
import type { WindowId, WorkspaceWindow } from "./types";

/**
 * "Go to window…" — what replaces the rail when the rail is hidden.
 *
 * Hiding chrome should cost discoverability, never capability, so every window
 * stays one summon away with the same numbers the rail shows. Digits pick
 * directly; the arrows and Enter are there for more windows than there are
 * digits.
 */

interface WindowSwitcherProps {
  readonly windows: readonly WorkspaceWindow[];
  readonly activeWindowId: WindowId;
  readonly busyWindows: ReadonlySet<WindowId>;
  readonly onPick: (id: WindowId) => void;
  readonly onCancel: () => void;
}

export const WindowSwitcher: React.FC<WindowSwitcherProps> = function WindowSwitcher({
  windows,
  activeWindowId,
  busyWindows,
  onPick,
  onCancel,
}) {
  const initial = Math.max(0, windows.findIndex((w) => w.id === activeWindowId));
  const [cursor, setCursor] = useState(initial);

  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      const digit = Number.parseInt(event.key, 10);
      if (!Number.isNaN(digit) && digit >= 1 && digit <= windows.length) {
        onPick(windows[digit - 1].id);
      } else if (event.key === "ArrowDown" || event.key === "ArrowRight") {
        setCursor((c) => (c + 1) % windows.length);
      } else if (event.key === "ArrowUp" || event.key === "ArrowLeft") {
        setCursor((c) => (c - 1 + windows.length) % windows.length);
      } else if (event.key === "Enter") {
        onPick(windows[cursor].id);
      } else if (event.key === "Escape") {
        onCancel();
      } else return;

      event.preventDefault();
      event.stopPropagation();
    },
    [cursor, onCancel, onPick, windows],
  );

  // Capture phase: the switcher owns the keyboard while it is up, ahead of any
  // pane's own bare-key bindings.
  useEffect(() => {
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [onKeyDown]);

  return createPortal(
    <div className="modal-backdrop wsp-opener-backdrop" onClick={onCancel}>
      <div
        className="modal-dialog-surface wsp-switcher"
        role="dialog"
        aria-modal="true"
        aria-label="Go to window"
        onClick={(event) => event.stopPropagation()}
      >
        {/* The digits are the point of this surface, so it says so up front
            rather than leaving them to be discovered by trying. */}
        <div className="wsp-switcher-head">
          <AppWindow size={13} aria-hidden="true" />
          <span>Go to window</span>
          {/* A range of one is not a range; with a single window the digit is
              already the only row on screen. */}
          {windows.length > 1 && (
            <span className="wsp-switcher-hint">
              <kbd>1</kbd>–<kbd>{Math.min(windows.length, 9)}</kbd>
            </span>
          )}
        </div>

        <div className="wsp-switcher-list">
          {windows.map((window, index) => (
            <button
              key={window.id}
              type="button"
              aria-current={window.id === activeWindowId ? "true" : undefined}
              className={
                "wsp-switcher-row" +
                (index === cursor ? " is-cursor" : "") +
                (window.id === activeWindowId ? " is-active" : "")
              }
              onMouseEnter={() => setCursor(index)}
              onClick={() => onPick(window.id)}
            >
              <span className="wsp-switcher-num">{index + 1}</span>
              <span className="wsp-switcher-name">{window.name}</span>
              {busyWindows.has(window.id) && (
                <span className="wsp-switcher-busy" aria-label="busy" />
              )}
            </button>
          ))}
        </div>
      </div>
    </div>,
    document.body,
  );
};
