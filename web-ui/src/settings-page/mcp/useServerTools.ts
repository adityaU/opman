import { useCallback, useEffect, useRef, useState } from "react";
import { fetchMcpServerTools, MCP_SERVERS_CHANGED, type McpCatalog } from "../../api/mcp";

/**
 * One server's tool catalog, fetched when the user opens it.
 *
 * Never on mount, and never for the whole list: a probe launches the server as a child
 * process, so listing every declared server at once would spawn one process per row for a
 * panel nobody had opened yet.
 *
 * The result is held for as long as the panel stays open and dropped when `mcp.json`
 * changes, because an edit to the entry is the one thing that can change the answer.
 */

export type ToolsState =
  | { readonly phase: "asking" }
  | { readonly phase: "answered"; readonly catalog: McpCatalog }
  /** The request itself failed — no network, no auth — as distinct from the probe failing. */
  | { readonly phase: "unreachable"; readonly reason: string };

export function useServerTools(name: string, open: boolean): {
  readonly state: ToolsState | undefined;
  readonly retry: () => void;
} {
  const [state, setState] = useState<ToolsState>();
  const alive = useRef(true);
  // Bumped to re-run the effect on an explicit retry, which a state reset alone would not
  // do: the effect's inputs are otherwise unchanged.
  const [attempt, setAttempt] = useState(0);

  const retry = useCallback(() => setAttempt((count) => count + 1), []);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    setState({ phase: "asking" });
    fetchMcpServerTools(name)
      .then((catalog) => alive.current && setState({ phase: "answered", catalog }))
      .catch((cause) => {
        if (!alive.current) return;
        setState({
          phase: "unreachable",
          reason: cause instanceof Error ? cause.message : String(cause),
        });
      });
  }, [name, open, attempt]);

  // An edit to the entry can change which tools the server has, or whether it starts at
  // all, so the open panel re-asks rather than showing the previous server's answer.
  useEffect(() => {
    if (!open) return;
    window.addEventListener(MCP_SERVERS_CHANGED, retry);
    return () => window.removeEventListener(MCP_SERVERS_CHANGED, retry);
  }, [open, retry]);

  return { state, retry };
}
