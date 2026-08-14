import { useCallback, useEffect, useMemo, useState } from "react";
import { ptyKill, ptyRename, ptySessions, spawnPty, type PtyKind, type PtySession } from "../api";
import { uuid } from "../utils/uuid";

/**
 * The shells running in the server process, as the UI sees them.
 *
 * Deliberately not cached per component: the list is shared state that any pane
 * — or an agent's MCP call — can add to, so every reader refetches rather than
 * trusting a copy. A tiny in-flight cache keeps concurrent readers to one
 * request, which is what makes "every reader refetches" cheap enough to mean.
 */

/** Fresh enough that a shell another pane just opened shows up. */
const STALE_MS = 1_500;

let cached: { at: number; shells: readonly PtySession[] } | null = null;
let inFlight: Promise<readonly PtySession[]> | null = null;

/**
 * Ids of shells started by this page.
 *
 * A shell created a moment ago is real on the server but may not be in the
 * copy of the list a component is still holding — and "not in the list" is how
 * an *exited* shell is recognised. This set is what lets a reader tell the two
 * apart until the fresh list lands.
 */
const minted = new Set<string>();

export function wasMintedHere(id: string): boolean {
  return minted.has(id);
}

/** Every live shell, from the cache when it is fresh and shared when it is not. */
export function loadShells(force = false): Promise<readonly PtySession[]> {
  const now = performance.now();
  if (!force && cached && now - cached.at < STALE_MS) return Promise.resolve(cached.shells);
  if (inFlight) return inFlight;

  inFlight = ptySessions()
    .then((shells) => {
      cached = { at: performance.now(), shells };
      return cached.shells;
    })
    .catch(() => cached?.shells ?? [])
    .finally(() => {
      inFlight = null;
    });
  return inFlight;
}

/** Drop the cache, so the next read sees a shell just started or killed. */
export function invalidateShells(): void {
  cached = null;
}

/** Whether a shell is still live. Used before attaching to a remembered id. */
export async function shellExists(id: string): Promise<boolean> {
  const shells = await loadShells(true);
  return shells.some((shell) => shell.id === id);
}

/**
 * Start a shell and return its id.
 *
 * The id is generated here rather than by the server so the caller can commit
 * it to its own state — a pane's persisted shell — before the request settles.
 */
export async function createShell(
  kind: PtyKind,
  project: string | null,
  sessionId?: string,
): Promise<string> {
  const id = uuid();
  minted.add(id);
  await spawnPty(kind, id, 24, 80, { project, sessionId });
  invalidateShells();
  return id;
}

export async function killShell(id: string): Promise<void> {
  await ptyKill(id).catch(() => {});
  invalidateShells();
}

export async function renameShell(id: string, label: string): Promise<void> {
  await ptyRename(id, label).catch(() => {});
  invalidateShells();
}

export interface Shells {
  /** Live shells for the project asked about, in a stable order. */
  readonly shells: readonly PtySession[];
  /** True until the first answer arrives, so an empty list is not shown early. */
  readonly loading: boolean;
  readonly refresh: () => void;
}

/**
 * Watch the shells belonging to one project.
 *
 * Polled rather than pushed: shells appear and vanish from outside this tab
 * (another pane, an agent, a program exiting), and there is no event for it.
 * The interval is only armed while a caller is mounted.
 */
export function useShells(project: string | null, pollMs = 3_000): Shells {
  const [shells, setShells] = useState<readonly PtySession[]>([]);
  const [loading, setLoading] = useState(true);

  const read = useCallback(async (force: boolean) => {
    const all = await loadShells(force);
    setShells(all);
    setLoading(false);
  }, []);

  useEffect(() => {
    let live = true;
    // Unforced: two pickers open a second apart share one request instead of
    // making two, which is the whole reason the cache has a lifetime.
    const tick = () => {
      if (live) void read(false);
    };
    tick();
    const timer = window.setInterval(tick, pollMs);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [pollMs, read]);

  const refresh = useCallback(() => {
    invalidateShells();
    void read(true);
  }, [read]);

  // Filtered here rather than server-side: one endpoint answers every pane, and
  // the list is short enough that filtering it is free.
  const forProject = useMemo(() => {
    if (!project) return shells;
    return shells.filter((shell) => shell.project === project);
  }, [project, shells]);

  return useMemo(() => ({ shells: forProject, loading, refresh }), [forProject, loading, refresh]);
}
