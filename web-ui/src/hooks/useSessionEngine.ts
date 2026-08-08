/**
 * What the composer is set to, scoped to the session it belongs to.
 *
 * A model, an agent, an effort tier and a permission mode all describe one conversation.
 * They used to be kept per *runner* instead — one shared record that whichever session was
 * touched last overwrote — so opening session B showed session A's model, and effort and
 * permission never followed a session at all.
 *
 * The values themselves are not stored here, or anywhere else in opman. Every runner
 * already keeps them per session and writes them to disk, because each one has to
 * reproduce the same configuration when it resumes a conversation. This hook reads them
 * off the session row and writes changes back to the runner that owns them.
 *
 * The per-runner memory ([`useRunnerConfig`]) keeps its one remaining job: seeding a
 * session that has never been configured, and the composer for a session that does not
 * exist yet.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import type { EngineChoices, ModelRef, SessionInfo } from "../api";
import { DEFAULT_PERMISSION, setSessionEngine } from "../api";
import type { ProviderCatalogue } from "../engine-picker/sessionEngine";
import { resolveModel } from "../engine-picker/sessionEngine";
import type { RunnerConfigStore } from "./useRunnerConfig";

export interface SessionEngineOptions {
  readonly activeSessionId: string | null;
  /** The active session's row from app state, or undefined before one is chosen. */
  readonly activeSession: SessionInfo | undefined;
  readonly currentRunner: string;
  readonly runnerConfig: RunnerConfigStore;
  readonly providers: ProviderCatalogue;
  /**
   * The model the active session's transcript was produced under, when it has one.
   *
   * The default runner's session list reports no configuration, so this is the only
   * per-session answer available for it. Null for a session with no assistant turn yet.
   */
  readonly transcriptModel: ModelRef | null;
  readonly selectedModel: ModelRef | null;
  readonly setSelectedModel: (next: ModelRef | null) => void;
  readonly selectedAgent: string;
  readonly setSelectedAgent: (next: string) => void;
}

export interface SessionEngine {
  readonly effort: string | null;
  readonly permission: string;
  readonly setModel: (next: ModelRef | null) => void;
  readonly setAgent: (next: string) => void;
  readonly setEffort: (next: string | null) => void;
  readonly setPermission: (next: string) => void;
}

export function useSessionEngine(options: SessionEngineOptions): SessionEngine {
  const {
    activeSessionId, activeSession, currentRunner, runnerConfig, providers, transcriptModel,
    selectedModel, setSelectedModel, selectedAgent, setSelectedAgent,
  } = options;

  const [effort, setEffortState] = useState<string | null>(null);
  const [permission, setPermissionState] = useState(DEFAULT_PERMISSION);

  // Read the moving parts through a ref so the setters keep one identity: the composer's
  // chip row is memoised on them, and a new identity per keystroke would re-render it.
  const latest = useRef({ activeSessionId, currentRunner });
  latest.current = { activeSessionId, currentRunner };

  // Apply the session's own configuration when the session or its runner changes.
  //
  // Keyed on identity, not on the values: re-running whenever the engine object changed
  // would fight a change the user just made, undoing it for as long as the round trip to
  // the runner takes. It replaces rather than fills, because carrying the previous
  // session's model into this one is the bug being fixed.
  const appliedFor = useRef<string>("");
  // Whether the model on screen came from the session itself or from the per-runner
  // memory standing in for it. Only a stand-in may be upgraded — see below.
  const modelIsStandIn = useRef(false);
  useEffect(() => {
    const key = `${activeSessionId ?? ""}::${currentRunner}`;
    // A session's configuration describes the runner it was made in. While a switch is
    // pending, the row still names the old one, and applying its model would offer the
    // new runner a model from another runner's catalogue.
    const ownRunner = !activeSession?.runner || activeSession.runner === currentRunner;
    const engine = ownRunner ? activeSession?.engine : undefined;
    // The default runner reports no configuration at all, so its sessions are read from
    // the transcript instead — the model an assistant message was produced under is the
    // model that session ran, which is the same fact by another route.
    const own = resolveModel(engine?.model, providers) ?? (ownRunner ? transcriptModel : null);

    if (appliedFor.current === key) {
      // The transcript arrives after the switch. Upgrading to it is safe only while the
      // model on screen is still the stand-in nobody chose.
      if (own && modelIsStandIn.current) {
        modelIsStandIn.current = false;
        setSelectedModel(own);
      }
      return;
    }
    // Wait for the catalogue: resolving the session's model needs it, and applying a null
    // in the meantime would let the repair effect overwrite the session's real choice.
    if (engine?.model && providers.all.length === 0) return;
    appliedFor.current = key;

    const remembered = runnerConfig.recall(currentRunner);
    modelIsStandIn.current = !own;
    const model = own ?? remembered.model;
    if (model) setSelectedModel(model);
    setSelectedAgent(engine?.agent ?? remembered.agent);
    setEffortState(engine?.effort ?? remembered.effort);
    setPermissionState(engine?.permissionMode ?? remembered.permission);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeSessionId, currentRunner, activeSession?.runner, activeSession?.engine, transcriptModel, providers.all.length]);

  /**
   * Record a change: with the runner that owns the session, and in the per-runner memory
   * that seeds the next new session. A session that does not exist yet has only the
   * second — there is no runner session to tell.
   */
  const record = useCallback((patch: Partial<EngineChoices>, remembered: Parameters<RunnerConfigStore["remember"]>[1]) => {
    const { activeSessionId: sessionId, currentRunner: runner } = latest.current;
    runnerConfig.remember(runner, remembered);
    if (!sessionId) return;
    // A rejected or failed write is not worth interrupting the user for: the same values
    // ride along with the next send, which is the path that always worked.
    void setSessionEngine(sessionId, patch).catch(() => {});
  }, [runnerConfig]);

  const setModel = useCallback((next: ModelRef | null) => {
    setSelectedModel(next);
    if (!next) return;
    // A chosen model is never a stand-in, so a transcript arriving late must not replace
    // it — the user's pick outranks what the session last happened to run under.
    modelIsStandIn.current = false;
    record({ model: next.modelID }, { model: next });
  }, [record, setSelectedModel]);

  const setAgent = useCallback((next: string) => {
    setSelectedAgent(next);
    if (!next) return;
    record({ agent: next }, { agent: next });
  }, [record, setSelectedAgent]);

  const setEffort = useCallback((next: string | null) => {
    setEffortState(next);
    // Effort is the one field whose `null` is a real choice — "the runner's default" —
    // so it is recorded rather than skipped.
    record(next ? { effort: next } : {}, { effort: next });
  }, [record]);

  const setPermission = useCallback((next: string) => {
    setPermissionState(next);
    if (!next) return;
    record({ permissionMode: next }, { permission: next });
  }, [record]);

  // Keep the per-runner memory current with what the composer actually shows, so a new
  // session opens on the configuration the user last worked in rather than on the last
  // one they explicitly clicked.
  useEffect(() => {
    if (!selectedModel && !selectedAgent) return;
    runnerConfig.remember(currentRunner, { model: selectedModel ?? undefined, agent: selectedAgent });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentRunner, selectedAgent, selectedModel]);

  return { effort, permission, setModel, setAgent, setEffort, setPermission };
}
