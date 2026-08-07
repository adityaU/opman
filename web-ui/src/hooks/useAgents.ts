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
  const hit = cache.get(runner);
  if (hit && Date.now() - hit.fetchedAt < CACHE_TTL_MS) return Promise.resolve(hit.agents);

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

export interface AgentCache {
  readonly agents: AgentInfo[];
  readonly loading: boolean;
}

export function useAgents(runner: string): AgentCache {
  const hit = cache.get(runner);
  const fresh = hit && Date.now() - hit.fetchedAt < CACHE_TTL_MS;
  const [agents, setAgents] = useState<AgentInfo[]>(fresh ? hit.agents : []);
  const [loading, setLoading] = useState(!fresh);

  useEffect(() => {
    let cancelled = false;
    const cached = cache.get(runner);
    if (cached && Date.now() - cached.fetchedAt < CACHE_TTL_MS) {
      setAgents(cached.agents);
      setLoading(false);
      return;
    }
    setLoading(true);
    void load(runner).then((next) => {
      if (cancelled) return;
      setAgents(next);
      setLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, [runner]);

  return { agents, loading };
}
