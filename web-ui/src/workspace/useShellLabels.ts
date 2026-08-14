/**
 * The running shells, and what to call one.
 *
 * Two surfaces need this list: the widget opener, which offers the shells a
 * terminal pane can attach to, and a pane's history, which has to name the shell
 * behind an entry as something better than a uuid. Fetched rather than derived
 * from the layout, because a shell is workspace-wide — it may have been started
 * by another pane, by an agent, or in another browser tab — so the only honest
 * list is the server's.
 *
 * One cache shared by both, refreshed when a surface that shows it opens. Two
 * copies would mean two fetches and two answers to "what is this shell called".
 */

import { useCallback, useMemo, useRef, useState } from "react";
import { loadShells } from "../terminal-panel/useShells";
import type { PtySession } from "../api/pty";

export interface ShellLabels {
  readonly shells: readonly PtySession[];
  /** Re-read the server's list. Safe to call on every open. */
  readonly refresh: () => void;
  /** The shell's name, or null when it is gone or was never named. */
  readonly labelOf: (ptyId: string | null) => string | null;
}

export function useShellLabels(): ShellLabels {
  const [shells, setShells] = useState<readonly PtySession[]>([]);
  // A fetch in flight is not repeated: opening the menu and the opener in quick
  // succession is one list, and the second request would only race the first.
  const pending = useRef(false);

  const refresh = useCallback(() => {
    if (pending.current) return;
    pending.current = true;
    void loadShells(true)
      .then(setShells)
      .finally(() => {
        pending.current = false;
      });
  }, []);

  const labelOf = useCallback(
    (ptyId: string | null) =>
      ptyId ? shells.find((shell) => shell.id === ptyId)?.label ?? null : null,
    [shells],
  );

  return useMemo(() => ({ shells, refresh, labelOf }), [labelOf, refresh, shells]);
}
