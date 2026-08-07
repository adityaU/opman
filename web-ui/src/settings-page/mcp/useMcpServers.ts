import { useCallback, useEffect, useRef, useState } from "react";
import {
  deleteMcpServer,
  fetchMcpServers,
  MCP_SERVERS_CHANGED,
  saveMcpServer,
  setMcpServerEnabled,
  type McpServer,
  type McpServerDraft,
} from "../../api/mcp";

/**
 * The declared MCP servers, kept in step with `mcp.json`.
 *
 * Every write on the backend reloads the registry and broadcasts, so this refetches on
 * that event rather than trusting its own optimistic copy. The one exception is the
 * enable toggle, which is applied locally first: it is the only control fast enough that a
 * round trip would read as lag.
 */

export interface McpServersState {
  readonly servers: readonly McpServer[];
  readonly loading: boolean;
  readonly error: string | undefined;
  /** Name currently being written, so its row can show it rather than the whole list. */
  readonly busy: string | undefined;
  readonly refresh: () => void;
  readonly toggle: (server: McpServer) => Promise<void>;
  readonly save: (name: string, draft: McpServerDraft) => Promise<boolean>;
  readonly remove: (name: string) => Promise<void>;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useMcpServers(onError: (message: string) => void): McpServersState {
  const [servers, setServers] = useState<readonly McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState<string>();
  const alive = useRef(true);

  const refresh = useCallback(() => {
    fetchMcpServers()
      .then((list) => {
        if (!alive.current) return;
        setServers(list);
        setError(undefined);
      })
      .catch((cause) => alive.current && setError(message(cause)))
      .finally(() => alive.current && setLoading(false));
  }, []);

  useEffect(() => {
    alive.current = true;
    refresh();
    // A finished OAuth login and an edit from another browser tab both arrive this way.
    window.addEventListener(MCP_SERVERS_CHANGED, refresh);
    return () => {
      alive.current = false;
      window.removeEventListener(MCP_SERVERS_CHANGED, refresh);
    };
  }, [refresh]);

  /** Run a write, reporting failure and leaving the list to the broadcast that follows. */
  const write = useCallback(
    async (name: string, action: () => Promise<void>): Promise<boolean> => {
      setBusy(name);
      try {
        await action();
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
    async (server: McpServer) => {
      const next = !server.enabled;
      setServers((list) =>
        list.map((entry) => (entry.name === server.name ? { ...entry, enabled: next } : entry)),
      );
      await write(server.name, () => setMcpServerEnabled(server.name, next));
    },
    [write],
  );

  const save = useCallback(
    (name: string, draft: McpServerDraft) => write(name, () => saveMcpServer(name, draft)),
    [write],
  );

  const remove = useCallback(
    async (name: string) => {
      await write(name, () => deleteMcpServer(name));
    },
    [write],
  );

  return { servers, loading, error, busy, refresh, toggle, save, remove };
}
