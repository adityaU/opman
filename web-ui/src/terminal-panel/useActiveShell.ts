import { useCallback, useEffect, useRef, useState } from "react";
import { createShell, killShell, renameShell, useShells, wasMintedHere } from "./useShells";
import type { PtyKind, PtySession } from "./types";

/**
 * Which shell a terminal is showing, and the moves that change it.
 *
 * The panel is told a shell id by its owner and reports back the one it settled
 * on, so the choice survives a reload without the panel owning any of it. Two
 * cases need deciding here rather than by the owner:
 *
 *  - the id is `null`, so the picker is shown;
 *  - the id names a shell that has since exited, which must not silently become
 *    a fresh shell — the user's build finished or crashed and they should see
 *    the list rather than an empty prompt pretending to be the same terminal.
 */

export interface ActiveShell {
  readonly ptyId: string | null;
  readonly shell: PtySession | null;
  readonly shells: readonly PtySession[];
  readonly loading: boolean;
  /** True when the picker should be shown instead of a terminal. */
  readonly choosing: boolean;
  readonly select: (ptyId: string) => void;
  readonly create: (kind: PtyKind) => void;
  readonly kill: (ptyId: string) => void;
  readonly renameById: (ptyId: string, label: string) => void;
  readonly startChoosing: () => void;
  readonly stopChoosing: () => void;
  /** Move to the next or previous shell in this project. */
  readonly step: (delta: number) => void;
}

export function useActiveShell(
  ptyId: string | null,
  projectPath: string | null,
  sessionId: string | null,
  onPtyIdChanged?: (ptyId: string | null) => void,
): ActiveShell {
  const { shells, loading, refresh } = useShells(projectPath);
  const [chosen, setChosen] = useState<string | null>(ptyId);
  const [switching, setSwitching] = useState(false);

  // Follow the owner when it names a different shell — a widget restored from
  // the layout, or one the workspace re-pointed at another shell.
  useEffect(() => setChosen(ptyId), [ptyId]);

  const report = useRef(onPtyIdChanged);
  report.current = onPtyIdChanged;

  const commit = useCallback((next: string | null) => {
    setChosen(next);
    setSwitching(false);
    report.current?.(next);
  }, []);

  // Ids this hook has seen alive. Needed to tell "exited" from "so new the
  // list in hand predates it" — only the former should be forgotten.
  const seenAlive = useRef<Set<string>>(new Set());

  /**
   * Forget a shell that is no longer running.
   *
   * Only once the list has actually loaded: before that every id looks dead,
   * and dropping it would send a restored pane to the picker on every reload.
   * A shell minted by this page is likewise spared until it has shown up once.
   */
  useEffect(() => {
    if (loading || !chosen) return;
    if (shells.some((shell) => shell.id === chosen)) {
      seenAlive.current.add(chosen);
      return;
    }
    if (wasMintedHere(chosen) && !seenAlive.current.has(chosen)) return;
    commit(null);
  }, [chosen, commit, loading, shells]);

  const select = useCallback((next: string) => commit(next), [commit]);

  const create = useCallback(
    (kind: PtyKind) => {
      void createShell(kind, projectPath, sessionId ?? undefined)
        .then((id) => {
          commit(id);
          refresh();
        })
        .catch(() => refresh());
    },
    [commit, projectPath, refresh, sessionId],
  );

  const kill = useCallback(
    (id: string) => {
      void killShell(id).then(() => {
        refresh();
        // Only the shell being shown needs a new answer; killing another
        // project's or another pane's shell leaves this one alone.
        if (id === chosen) commit(null);
      });
    },
    [chosen, commit, refresh],
  );

  const renameById = useCallback(
    (id: string, label: string) => {
      const trimmed = label.trim();
      if (!trimmed) return;
      void renameShell(id, trimmed).then(refresh);
    },
    [refresh],
  );

  const step = useCallback(
    (delta: number) => {
      if (shells.length === 0) return;
      const index = shells.findIndex((shell) => shell.id === chosen);
      const next = index < 0 ? 0 : (index + delta + shells.length) % shells.length;
      const target = shells[next];
      if (target) commit(target.id);
    },
    [chosen, commit, shells],
  );

  const shell = chosen ? shells.find((candidate) => candidate.id === chosen) ?? null : null;

  return {
    ptyId: chosen,
    shell,
    shells,
    loading,
    choosing: switching || chosen === null,
    select,
    create,
    kill,
    renameById,
    startChoosing: useCallback(() => setSwitching(true), []),
    stopChoosing: useCallback(() => setSwitching(false), []),
    step,
  };
}
