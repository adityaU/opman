import { useState, useEffect, useCallback, useRef } from "react";
import { fetchProviders } from "../api";
import type { Provider } from "../types";

export interface ProviderCache {
  all: Provider[];
  connected: Set<string>;
  defaults: Record<string, string>;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

/**
 * Singleton-cached provider data. Fetches once on first mount,
 * then returns the same data for all subsequent consumers.
 * Call `refresh()` to force a re-fetch (e.g. after provider config changes).
 */

let globalCache: Record<string, {
  all: Provider[];
  connected: string[];
  defaults: Record<string, string>;
  fetchedAt: number;
}> = {};

const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

export function useProviders(runner = "opencode"): ProviderCache {
  const cached = globalCache[runner];
  const [all, setAll] = useState<Provider[]>(cached?.all ?? []);
  const [connected, setConnected] = useState<Set<string>>(
    new Set(cached?.connected ?? [])
  );
  const [defaults, setDefaults] = useState<Record<string, string>>(
    cached?.defaults ?? {}
  );
  const [loading, setLoading] = useState(!cached);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const load = useCallback(
    (force = false) => {
      // Use cache if fresh enough
      if (
        !force &&
        globalCache[runner] &&
        Date.now() - globalCache[runner].fetchedAt < CACHE_TTL_MS
      ) {
        setAll(globalCache[runner].all);
        setConnected(new Set(globalCache[runner].connected));
        setDefaults(globalCache[runner].defaults);
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);
      fetchProviders(runner)
        .then((resp) => {
          if (!mountedRef.current) return;
          globalCache[runner] = {
            all: resp.all,
            connected: resp.connected,
            defaults: resp.default,
            fetchedAt: Date.now(),
          };
          setAll(resp.all);
          setConnected(new Set(resp.connected));
          setDefaults(resp.default);
        })
        .catch((e) => {
          if (!mountedRef.current) return;
          setError(e instanceof Error ? e.message : "Failed to fetch providers");
        })
        .finally(() => {
          if (mountedRef.current) setLoading(false);
        });
    },
    [runner]
  );

  useEffect(() => {
    mountedRef.current = true;
    load();
    return () => {
      mountedRef.current = false;
    };
  }, [load]);

  const refresh = useCallback(() => load(true), [load]);

  return { all, connected, defaults, loading, error, refresh };
}

/** Invalidate the global provider cache (e.g. after model change) */
export function invalidateProviderCache() {
  globalCache = {};
}
