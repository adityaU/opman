/**
 * useServers — one cached copy of `/api/servers` shared by every caller.
 *
 *   const { servers, current, multi, loading, error, refresh } = useServers();
 *
 * - `servers`: home first, then remotes, with each one's reachability.
 * - `current`: the entry for the server this instance talks to (`SERVER_ID`), if listed.
 * - `multi`: more than one server exists. Project pickers group by server only then.
 * - `refresh()`: refetch; every mounted caller re-renders with the new list. Call it
 *   after adding, editing or removing a server.
 *
 * The first mount fetches; later mounts reuse the cache. Before the first answer the
 * list is `[]` and `multi` is false, so callers render their single-server shape.
 */
import { useCallback, useEffect, useSyncExternalStore } from "react";
import { SERVER_ID } from "../api/base";
import { listServers, type ServerInfo } from "../api/servers";

interface Snapshot {
  readonly servers: readonly ServerInfo[];
  readonly loading: boolean;
  readonly loaded: boolean;
  readonly error: string | null;
}

let snapshot: Snapshot = { servers: [], loading: false, loaded: false, error: null };
let inflight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function publish(next: Snapshot): void {
  snapshot = next;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Fetch the list into the shared cache. Concurrent calls share one request. */
export function refreshServers(): Promise<void> {
  if (inflight) return inflight;
  publish({ ...snapshot, loading: true });
  inflight = listServers()
    .then((servers) => publish({ servers, loading: false, loaded: true, error: null }))
    .catch((err: unknown) => {
      const error = err instanceof Error ? err.message : "Could not load servers";
      publish({ ...snapshot, loading: false, loaded: true, error });
    })
    .finally(() => {
      inflight = null;
    });
  return inflight;
}

/** Test seam: forget the cache. */
export function resetServersCache(): void {
  inflight = null;
  snapshot = { servers: [], loading: false, loaded: false, error: null };
}

export interface ServersState {
  readonly servers: readonly ServerInfo[];
  readonly current: ServerInfo | undefined;
  readonly multi: boolean;
  readonly loading: boolean;
  readonly error: string | null;
  readonly refresh: () => Promise<void>;
}

export function useServers(): ServersState {
  const state = useSyncExternalStore(subscribe, () => snapshot);

  useEffect(() => {
    if (snapshot.loaded || inflight) return;
    void refreshServers();
  }, []);

  const refresh = useCallback(() => refreshServers(), []);

  return {
    servers: state.servers,
    current: state.servers.find((server) => server.id === SERVER_ID),
    multi: state.servers.length > 1,
    loading: state.loading,
    error: state.error,
    refresh,
  };
}
