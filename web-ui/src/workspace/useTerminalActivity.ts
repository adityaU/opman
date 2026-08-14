import { useEffect, useMemo, useRef, useState } from "react";
import { ptySessions } from "../api";
import { panes } from "./tree";
import type { PaneId, WorkspaceWindow } from "./types";

/**
 * Which terminal panes are running a foreground command.
 *
 * Polled from one endpoint covering every shell rather than read off each
 * terminal's output stream. A pane in a background window is not mounted and
 * has no stream, but its shell is still running the build — and that is exactly
 * the case the window rail's pulse exists to surface.
 *
 * The poll is conditional: with no terminal panes anywhere, no request is made.
 */

/** Slow enough to be free, fast enough that a command reads as started. */
const POLL_MS = 2000;

const NONE: ReadonlySet<PaneId> = new Set();

export function useTerminalActivity(windows: readonly WorkspaceWindow[]): ReadonlySet<PaneId> {
  const owners = useMemo(() => terminalPanes(windows), [windows]);

  // Read through a ref so a layout edit does not restart the timer; only a
  // change to the *set of shells* should, and that is what `key` tracks.
  const ownersRef = useRef(owners);
  ownersRef.current = owners;
  const key = useMemo(() => [...owners.values()].sort().join("|"), [owners]);

  const [busy, setBusy] = useState<ReadonlySet<PaneId>>(NONE);

  useEffect(() => {
    if (!key) {
      setBusy((current) => (current.size === 0 ? current : NONE));
      return;
    }

    let live = true;
    const poll = async () => {
      const sessions = await ptySessions().catch(() => null);
      if (!live || !sessions) return;
      const running = new Set(
        sessions.filter((s) => s.activity === "running").map((s) => s.id),
      );
      const next = new Set<PaneId>();
      for (const [pane, ptyId] of ownersRef.current) {
        if (running.has(ptyId)) next.add(pane);
      }
      setBusy((current) => (sameSet(current, next) ? current : next));
    };

    void poll();
    const timer = window.setInterval(poll, POLL_MS);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [key]);

  return busy;
}

/** Every terminal pane showing a shell, mapped to that shell. */
function terminalPanes(windows: readonly WorkspaceWindow[]): Map<PaneId, string> {
  const owners = new Map<PaneId, string>();
  for (const window of windows) {
    for (const pane of panes(window.root)) {
      if (pane.widget?.kind !== "terminal" || !pane.widget.ptyId) continue;
      owners.set(pane.id, pane.widget.ptyId);
    }
  }
  return owners;
}

function sameSet(a: ReadonlySet<PaneId>, b: ReadonlySet<PaneId>): boolean {
  if (a.size !== b.size) return false;
  for (const value of a) {
    if (!b.has(value)) return false;
  }
  return true;
}
