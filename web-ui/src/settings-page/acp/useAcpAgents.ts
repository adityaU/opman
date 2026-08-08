import { useCallback, useEffect, useRef, useState } from "react";
import {
  ACP_AGENTS_CHANGED,
  deleteAcpAgent,
  fetchAcpAgents,
  saveAcpAgent,
  setAcpAgentEnabled,
  type AcpAgent,
  type AcpAgentDraft,
  type AcpWriteResult,
} from "../../api/acp";

/**
 * The declared ACP agents, kept in step with `acp.json`.
 *
 * Every write reconciles the live engines on the backend and broadcasts, so this refetches
 * on that event rather than trusting its own optimistic copy — which matters more here than
 * for MCP servers, because a write can *fail to take effect* while still succeeding:
 * an agent whose runner slot is held by another engine saves fine and never starts. That is
 * what `notice` carries.
 */

export interface AcpAgentsState {
  readonly agents: readonly AcpAgent[];
  readonly loading: boolean;
  readonly error: string | undefined;
  /** Id currently being written, so its row can show it rather than the whole list. */
  readonly busy: string | undefined;
  /** What the last write did to the running set, when it is worth saying. */
  readonly notice: string | undefined;
  readonly dismissNotice: () => void;
  readonly refresh: () => void;
  readonly toggle: (agent: AcpAgent) => Promise<void>;
  readonly save: (id: string, draft: AcpAgentDraft) => Promise<boolean>;
  readonly remove: (id: string) => Promise<void>;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Say what changed, and only when it is not already obvious from the list redrawing.
 *
 * A blocked agent is the case worth a sentence: the file now says it exists, the row shows
 * it, and nothing is running — without this the page would look like it worked.
 */
function describe(result: AcpWriteResult): string | undefined {
  if (result.blocked.length > 0) {
    return `Saved, but ${result.blocked.join(", ")} did not start — its runner slot is served by another engine. Give it a different slot.`;
  }
  if (result.deferred.length > 0) {
    return `Saved. ${result.deferred.join(", ")} keeps running its current definition until opman restarts — it is the engine opman itself was started on.`;
  }
  if (result.started.length > 0) {
    return `${result.started.join(", ")} is running and can be picked for a new session.`;
  }
  if (result.stopped.length > 0) {
    return `${result.stopped.join(", ")} stopped. Sessions already in it stay on disk.`;
  }
  return undefined;
}

export function useAcpAgents(onError: (message: string) => void): AcpAgentsState {
  const [agents, setAgents] = useState<readonly AcpAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const alive = useRef(true);

  const refresh = useCallback(() => {
    fetchAcpAgents()
      .then((list) => {
        if (!alive.current) return;
        setAgents(list);
        setError(undefined);
      })
      .catch((cause) => alive.current && setError(message(cause)))
      .finally(() => alive.current && setLoading(false));
  }, []);

  useEffect(() => {
    alive.current = true;
    refresh();
    // An edit from another browser tab arrives this way, as does opman's own reconcile.
    window.addEventListener(ACP_AGENTS_CHANGED, refresh);
    return () => {
      alive.current = false;
      window.removeEventListener(ACP_AGENTS_CHANGED, refresh);
    };
  }, [refresh]);

  /** Run a write, reporting failure and leaving the list to the broadcast that follows. */
  const write = useCallback(
    async (id: string, action: () => Promise<AcpWriteResult>): Promise<boolean> => {
      setBusy(id);
      try {
        const result = await action();
        if (alive.current) setNotice(describe(result));
        return true;
      } catch (cause) {
        onError(message(cause));
        refresh();
        return false;
      } finally {
        if (alive.current) setBusy(undefined);
      }
    },
    [onError, refresh],
  );

  const toggle = useCallback(
    async (agent: AcpAgent) => {
      const next = !agent.enabled;
      // Applied locally first: the switch is the one control fast enough that waiting for
      // a round trip would read as lag.
      setAgents((list) =>
        list.map((entry) => (entry.id === agent.id ? { ...entry, enabled: next } : entry)),
      );
      await write(agent.id, () => setAcpAgentEnabled(agent.id, next));
    },
    [write],
  );

  const save = useCallback(
    (id: string, draft: AcpAgentDraft) => write(id, () => saveAcpAgent(id, draft)),
    [write],
  );

  const remove = useCallback(
    async (id: string) => {
      await write(id, () => deleteAcpAgent(id));
    },
    [write],
  );

  const dismissNotice = useCallback(() => setNotice(undefined), []);

  return { agents, loading, error, busy, notice, dismissNotice, refresh, toggle, save, remove };
}
