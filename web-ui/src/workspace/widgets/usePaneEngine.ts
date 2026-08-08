import { useCallback, useMemo, useRef, useState } from "react";
import { DEFAULT_PERMISSION, setSessionEngine } from "../../api";
import type { EngineChoices, SessionInfo } from "../../api";
import { engineFromSession } from "../../engine-picker/sessionEngine";
import { useProviders } from "../../hooks/useProviders";
import { useWorkspaceChat } from "./WorkspaceChatContext";
import type { PaneEngine } from "../types";

/**
 * A chat pane's engine, and the five setters the composer's chip row needs.
 *
 * A pane shows, in order: the engine it was given explicitly, the configuration of the
 * session it is bound to, then the shell's. The middle one is what makes two panes on two
 * sessions show two different models — without it both showed the shell's, so opening a
 * second session in a second pane displayed the first one's setup.
 *
 * A change is written to the runner that owns the session, because that is where these
 * values live: every runner keeps them per session and persists them. The per-pane copy
 * is kept alongside so a pane the user has deliberately configured differently from its
 * session stays that way across a reload.
 *
 * The runner is the one field a send has to treat specially. Naming a runner on a message
 * to an existing session is upstream's "switch engines" request, and it forks the
 * conversation into a handoff — so it must be named exactly once, on the first send after
 * the user actually changed it, and never again.
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

export function usePaneEngine(
  paneId: string,
  own: PaneEngine | null,
  sessionId: string | null,
): PaneEngineControls {
  const services = useWorkspaceChat();

  const session = useMemo<SessionInfo | undefined>(() => {
    if (!sessionId) return undefined;
    for (const project of services.appState?.projects ?? []) {
      const found = project.sessions?.find((row) => row.id === sessionId);
      if (found) return found;
    }
    return undefined;
  }, [services.appState, sessionId]);

  // The pane's own choice wins: it is the one the user made about this pane. Only a pane
  // that has never been configured asks the session what it runs as.
  const runner = own?.runner ?? session?.runner ?? services.defaultEngine.runner;
  const providers = useProviders(runner);
  const engine = useMemo(
    () => own ?? engineFromSession(session, runner, providers, services.defaultEngine),
    [own, providers, runner, services.defaultEngine, session],
  );

  // A pending runner switch is transient, not part of the layout: a reload
  // means the pane never sent, and the session it would have switched either
  // does not exist yet or was never told.
  const [switchRunner, setSwitchRunner] = useState(false);

  // Read the live engine through a ref so the setters keep one identity — the
  // composer's chip row is memoised on them.
  const latest = useRef({ engine, sessionId });
  latest.current = { engine, sessionId };

  const patch = useCallback(
    (change: Partial<PaneEngine>, choices: Partial<EngineChoices>) => {
      services.setEngine(paneId, { ...latest.current.engine, ...change });
      const target = latest.current.sessionId;
      // A failed write is not worth interrupting the user for: the same values ride along
      // with the next send, which is the path that always worked.
      if (target) void setSessionEngine(target, choices).catch(() => {});
    },
    [paneId, services],
  );

  const setRunner = useCallback(
    (runner: string) => {
      if (runner === latest.current.engine.runner) return;
      // Model, agent, effort and permission all name things inside one runner's
      // catalogue, so they are cleared rather than carried across. The composer
      // repairs them against the new runner's own options.
      //
      // Nothing is sent to the old runner: the choice being made is to leave it.
      services.setEngine(paneId, {
        runner,
        model: null,
        agent: "",
        effort: null,
        permission: DEFAULT_PERMISSION,
      });
      setSwitchRunner(true);
    },
    [paneId, services],
  );

  const setModel = useCallback(
    (modelID: string, providerID: string) =>
      patch({ model: { providerID, modelID } }, { model: modelID }),
    [patch],
  );
  const setAgent = useCallback((agent: string) => patch({ agent }, { agent }), [patch]);
  const setEffort = useCallback(
    (effort: string | null) => patch({ effort }, effort ? { effort } : {}),
    [patch],
  );
  const setPermission = useCallback(
    (permission: string) => patch({ permission }, { permissionMode: permission }),
    [patch],
  );
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
