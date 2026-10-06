import { useEffect, useState } from "react";
import { fetchAgents } from "../api";
import type { AgentInfo } from "../api";

/**
 * A runner's agents, cached per runner.
 *
 * Cached for the same reason `useProviders` is: the agent list is now read
 * wherever an engine chip is mounted rather than only when the palette is
 * open, and a workspace can hold four composers at once. Without this, every
 * one of them would issue its own `/agents?runner=` on mount and again on
 * every runner change.
 *
 * Same 5-minute TTL as the provider cache, so the two go stale together.
 */

const CACHE_TTL_MS = 5 * 60 * 1000;

const cache = new Map<string, { agents: AgentInfo[]; fetchedAt: number }>();
/** In-flight requests, so N simultaneous mounts share one round trip. */
const inFlight = new Map<string, Promise<AgentInfo[]>>();

function load(runner: string): Promise<AgentInfo[]> {
  const hit = fresh(runner);
  if (hit) return Promise.resolve(hit);

  const pending = inFlight.get(runner);
  if (pending) return pending;

  const request = fetchAgents(runner)
    .then((agents) => {
      cache.set(runner, { agents, fetchedAt: Date.now() });
      return agents;
    })
    .catch(() => [] as AgentInfo[])
    .finally(() => inFlight.delete(runner));

  inFlight.set(runner, request);
  return request;
}

const NO_AGENTS: AgentInfo[] = [];

function fresh(runner: string): AgentInfo[] | null {
  const hit = cache.get(runner);
  return hit && Date.now() - hit.fetchedAt < CACHE_TTL_MS ? hit.agents : null;
}

export interface AgentCache {
  readonly agents: AgentInfo[];
  readonly loading: boolean;
}

export function useAgents(runner: string): AgentCache {
  // Tagged with the runner it describes: until the effect for a new runner has run, the
  // state still holds the previous runner's agents, and the agent repair in
  // `useEngineOptions` would "fix" the selection to one of them.
  const [loaded, setLoaded] = useState<{ runner: string; agents: AgentInfo[] } | null>(() => {
    const hit = fresh(runner);
    return hit ? { runner, agents: hit } : null;
  });

  useEffect(() => {
    let cancelled = false;
    const hit = fresh(runner);
    if (hit) {
      setLoaded({ runner, agents: hit });
      return;
    }
    void load(runner).then((agents) => {
      if (!cancelled) setLoaded({ runner, agents });
    });
    return () => {
      cancelled = true;
    };
  }, [runner]);

  if (loaded?.runner === runner) return { agents: loaded.agents, loading: false };
  const hit = fresh(runner);
  return hit ? { agents: hit, loading: false } : { agents: NO_AGENTS, loading: true };
}
