/**
 * What each runner was last configured with.
 *
 * Runners do not share a catalogue: a model, an agent, an effort tier and a
 * permission mode all mean something only inside one runner. Switching used to
 * throw that away — the effort and permission survived in memory for the
 * session, the model was reset to the runner's default, and a reload lost the
 * lot. So going Claude → Codex → Claude meant reconfiguring Claude from
 * scratch, every time.
 *
 * Writes happen on explicit user actions only, never from an effect watching
 * state, so restoring a config can never feed back into recording one.
 */
import { useCallback, useRef, useState } from "react";
import { DEFAULT_PERMISSION } from "../api/session";

export interface ModelRef {
  providerID: string;
  modelID: string;
}

export interface RunnerConfig {
  model: ModelRef | null;
  agent: string;
  effort: string | null;
  permission: string;
}

const STORAGE_KEY = "opman-runner-config";

export function emptyConfig(): RunnerConfig {
  return { model: null, agent: "", effort: null, permission: DEFAULT_PERMISSION };
}

/**
 * The persisted shape.
 *
 * v1 stored the per-runner map at the top level. The runner *choice* itself has
 * to live next to it — a new session with no runner of its own should open on
 * the engine the user last worked in — so the map moved under `runners`. The old
 * flat value is still in browsers, so a stored object without `runners` is read
 * as that map and rewritten in the new shape on the next write.
 */
interface StoredState {
  runners: Record<string, RunnerConfig>;
  lastRunner: string;
}

function readStored(): StoredState {
  const empty: StoredState = { runners: {}, lastRunner: "" };
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return empty;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return empty;
    const wrapped = (parsed as { runners?: unknown }).runners;
    if (!wrapped || typeof wrapped !== "object") {
      return { runners: parsed as Record<string, RunnerConfig>, lastRunner: "" };
    }
    const last = (parsed as { lastRunner?: unknown }).lastRunner;
    return {
      runners: wrapped as Record<string, RunnerConfig>,
      lastRunner: typeof last === "string" ? last : "",
    };
  } catch {
    // Corrupt or unavailable storage is not worth failing a render over.
    return empty;
  }
}

function persist(state: StoredState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage full or blocked — the in-memory state still works this session.
  }
}

export interface RunnerConfigStore {
  /** The remembered config for a runner, filled in with that runner's defaults. */
  recall: (runner: string) => RunnerConfig;
  /** Record part of a runner's config. Empty values are ignored, not stored. */
  remember: (runner: string, patch: Partial<RunnerConfig>) => void;
  /** Record the runner the user explicitly picked. */
  rememberRunner: (runner: string) => void;
  /** The last explicitly picked runner, or "" when there has never been one. */
  lastRunner: () => string;
}

export function useRunnerConfig(): RunnerConfigStore {
  const [state, setState] = useState<StoredState>(readStored);
  // Read at call time so the getters are stable identities and never re-render a
  // consumer just because another runner's config changed.
  const ref = useRef(state);
  ref.current = state;

  const recall = useCallback((runner: string): RunnerConfig => {
    const stored = ref.current.runners[runner];
    if (!stored) return emptyConfig();
    return { ...emptyConfig(), ...stored };
  }, []);

  const lastRunner = useCallback(() => ref.current.lastRunner, []);

  const rememberRunner = useCallback((runner: string) => {
    if (!runner) return;
    setState((current) => {
      if (current.lastRunner === runner) return current;
      const next = { ...current, lastRunner: runner };
      persist(next);
      return next;
    });
  }, []);

  const remember = useCallback((runner: string, patch: Partial<RunnerConfig>) => {
    if (!runner) return;
    // A cleared value means "the UI reset this", not "the user chose nothing" —
    // recording it would erase a good config on every session switch.
    const meaningful: Partial<RunnerConfig> = {};
    if (patch.model) meaningful.model = patch.model;
    if (patch.agent) meaningful.agent = patch.agent;
    if (patch.permission) meaningful.permission = patch.permission;
    // Effort is the exception: `null` is a real choice ("default effort").
    if ("effort" in patch) meaningful.effort = patch.effort ?? null;
    if (Object.keys(meaningful).length === 0) return;

    setState((current) => {
      const next: StoredState = {
        ...current,
        runners: {
          ...current.runners,
          [runner]: { ...emptyConfig(), ...current.runners[runner], ...meaningful },
        },
      };
      persist(next);
      return next;
    });
  }, []);

  return { recall, remember, rememberRunner, lastRunner };
}
