import { useCallback, useEffect, useRef, useState } from "react";
import { fetchAcpAuth, type AcpAgent, type AcpAuth } from "../../api/acp";

/**
 * Each enabled agent's login state: connected or not.
 *
 * Asking costs a short-lived process per agent, so an agent is asked once when it first
 * appears enabled and again only on request (after a login, or the recheck button) —
 * never on every render or every list refetch.
 */
export type AuthEntry =
  | { readonly status: "checking" }
  | { readonly status: "known"; readonly auth: AcpAuth }
  | { readonly status: "failed"; readonly message: string };

export interface AcpAuthState {
  readonly entries: Readonly<Record<string, AuthEntry>>;
  readonly recheck: (id: string) => void;
}

const CHECKING: AuthEntry = { status: "checking" };

function asked(agent: AcpAgent): boolean {
  return agent.enabled && agent.launchable && agent.command !== "";
}

export function useAcpAuth(agents: readonly AcpAgent[]): AcpAuthState {
  const [entries, setEntries] = useState<Record<string, AuthEntry>>({});
  const requested = useRef(new Set<string>());
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const ask = useCallback((id: string) => {
    requested.current.add(id);
    setEntries((current) => ({ ...current, [id]: CHECKING }));
    fetchAcpAuth(id)
      .then((auth): AuthEntry => ({ status: "known", auth }))
      .catch((cause): AuthEntry => ({
        status: "failed",
        message: cause instanceof Error ? cause.message : String(cause),
      }))
      .then((entry) => {
        if (alive.current) setEntries((current) => ({ ...current, [id]: entry }));
      });
  }, []);

  useEffect(() => {
    for (const agent of agents) {
      if (asked(agent) && !requested.current.has(agent.id)) ask(agent.id);
    }
  }, [agents, ask]);

  return { entries, recheck: ask };
}
