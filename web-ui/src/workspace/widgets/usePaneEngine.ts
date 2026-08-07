import { useCallback, useMemo, useRef, useState } from "react";
import { useWorkspaceChat } from "./WorkspaceChatContext";
import type { PaneEngine } from "../types";

/**
 * A chat pane's engine, and the five setters the composer's chip row needs.
 *
 * A pane starts with no engine of its own and shows the shell's. The first
 * change materialises one — copying the shell's current values and applying the
 * change on top — so touching the agent does not silently strand the pane on
 * whatever model happened to be selected at that moment.
 *
 * The runner is the one field a send has to treat specially. Naming a runner on
 * a message to an existing session is upstream's "switch engines" request, and
 * it forks the conversation into a handoff — so it must be named exactly once,
 * on the first send after the user actually changed it, and never again.
 */

export interface PaneEngineControls {
  readonly engine: PaneEngine;
  /** Whether the next send should name the runner. */
  readonly switchRunner: boolean;
  /** Call after a send that carried the runner. */
  readonly runnerSent: () => void;
  readonly setRunner: (runner: string) => void;
  readonly setModel: (modelId: string, providerId: string) => void;
  readonly setAgent: (agent: string) => void;
  readonly setEffort: (effort: string | null) => void;
  readonly setPermission: (permission: string) => void;
}

export function usePaneEngine(paneId: string, own: PaneEngine | null): PaneEngineControls {
  const services = useWorkspaceChat();
  const engine = own ?? services.defaultEngine;

  // A pending runner switch is transient, not part of the layout: a reload
  // means the pane never sent, and the session it would have switched either
  // does not exist yet or was never told.
  const [switchRunner, setSwitchRunner] = useState(false);

  // Read the live engine through a ref so the setters keep one identity — the
  // composer's chip row is memoised on them.
  const latest = useRef(engine);
  latest.current = engine;

  const patch = useCallback(
    (change: Partial<PaneEngine>) => services.setEngine(paneId, { ...latest.current, ...change }),
    [paneId, services],
  );

  const setRunner = useCallback(
    (runner: string) => {
      if (runner === latest.current.runner) return;
      // Model, agent, effort and permission all name things inside one runner's
      // catalogue, so they are cleared rather than carried across. The composer
      // repairs them against the new runner's own options.
      patch({ runner, model: null, agent: "", effort: null, permission: defaultPermission(runner) });
      setSwitchRunner(true);
    },
    [patch],
  );

  const setModel = useCallback(
    (modelID: string, providerID: string) => patch({ model: { providerID, modelID } }),
    [patch],
  );
  const setAgent = useCallback((agent: string) => patch({ agent }), [patch]);
  const setEffort = useCallback((effort: string | null) => patch({ effort }), [patch]);
  const setPermission = useCallback((permission: string) => patch({ permission }), [patch]);
  const runnerSent = useCallback(() => setSwitchRunner(false), []);

  return useMemo(
    () => ({
      engine,
      switchRunner,
      runnerSent,
      setRunner,
      setModel,
      setAgent,
      setEffort,
      setPermission,
    }),
    [engine, runnerSent, setAgent, setEffort, setModel, setPermission, setRunner, switchRunner],
  );
}

/** Codex asks before acting by default; every other runner does not. */
function defaultPermission(runner: string): string {
  return runner === "codex" ? "on-request" : "default";
}
