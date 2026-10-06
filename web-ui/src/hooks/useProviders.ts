import { useState, useEffect, useCallback, useRef } from "react";
import { fetchProviders } from "../api";
import type { PermissionModeOption } from "../api/session";
import type { Provider } from "../types";

export interface ProviderCache {
  all: Provider[];
  connected: Set<string>;
  defaults: Record<string, string>;
  /** Permission modes the engine reported, when it reports its own (ACP agents do). */
  permissionModes: PermissionModeOption[] | null;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

/**
 * Provider data, cached per runner and shared by every consumer.
 * Call `refresh()` to force a re-fetch (e.g. after provider config changes).
 *
 * A catalogue belongs to exactly one runner, and everything the hook returns is read
 * against the runner it is asked about *now*. It used to keep the lists in plain state and
 * let any answer write them, so two things leaked one runner's models into another's
 * picker: the previous runner's lists stayed on screen while the new one loaded, and a
 * slow answer for the previous runner (an ACP `/provider` read waits for the agent's
 * startup probe) landed after the switch and replaced the new runner's lists outright.
 * The model repair in `useEngineOptions` then "fixed" the selection to a model from the
 * wrong runner and wrote it to the session.
 */

interface CatalogueEntry {
  all: Provider[];
  connected: Set<string>;
  defaults: Record<string, string>;
  permissionModes: PermissionModeOption[] | null;
  fetchedAt: number;
}

/** What the hook last learned, and for which runner. */
interface Loaded {
  runner: string;
  entry: CatalogueEntry | null;
  loading: boolean;
  error: string | null;
}

let globalCache: Record<string, CatalogueEntry> = {};

const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

const EMPTY: CatalogueEntry = {
  all: [],
  connected: new Set(),
  defaults: {},
  permissionModes: null,
  fetchedAt: 0,
};

function freshEntry(runner: string): CatalogueEntry | null {
  const hit = globalCache[runner];
  return hit && Date.now() - hit.fetchedAt < CACHE_TTL_MS ? hit : null;
}

export function useProviders(runner = "opencode"): ProviderCache {
  const [loaded, setLoaded] = useState<Loaded>(() => {
    const entry = globalCache[runner] ?? null;
    return { runner, entry, loading: !entry, error: null };
  });
  // Identifies the request whose answer may still be shown. Bumped by every new load and
  // by unmount, so an older answer can fill the cache but never the hook's state.
  const requestRef = useRef(0);

  const load = useCallback(
    (force = false) => {
      const request = ++requestRef.current;
      const cached = force ? null : freshEntry(runner);
      if (cached) {
        setLoaded({ runner, entry: cached, loading: false, error: null });
        return;
      }

      setLoaded({ runner, entry: globalCache[runner] ?? null, loading: true, error: null });
      fetchProviders(runner)
        .then((resp) => {
          const entry: CatalogueEntry = {
            all: resp.all,
            connected: new Set(resp.connected),
            defaults: resp.default,
            permissionModes: resp.permissionModes ?? null,
            fetchedAt: Date.now(),
          };
          globalCache[runner] = entry;
          if (request !== requestRef.current) return;
          setLoaded({ runner, entry, loading: false, error: null });
        })
        .catch((e) => {
          if (request !== requestRef.current) return;
          const error = e instanceof Error ? e.message : "Failed to fetch providers";
          setLoaded((prev) => ({ ...prev, runner, loading: false, error }));
        });
    },
    [runner]
  );

  useEffect(() => {
    load();
    return () => {
      requestRef.current += 1;
    };
  }, [load]);

  const refresh = useCallback(() => load(true), [load]);

  // Until the effect for a new runner has run, `loaded` still describes the previous one.
  const current: Loaded = loaded.runner === runner
    ? loaded
    : { runner, entry: globalCache[runner] ?? null, loading: !freshEntry(runner), error: null };
  const entry = current.entry ?? EMPTY;

  return {
    all: entry.all,
    connected: entry.connected,
    defaults: entry.defaults,
    permissionModes: entry.permissionModes,
    loading: current.loading,
    error: current.error,
    refresh,
  };
}

/** Invalidate the global provider cache (e.g. after model change) */
export function invalidateProviderCache() {
  globalCache = {};
}
