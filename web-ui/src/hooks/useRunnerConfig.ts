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

function defaultPermission(runner: string): string {
  return runner === "codex" ? "on-request" : "default";
}

export function emptyConfig(runner: string): RunnerConfig {
  return { model: null, agent: "", effort: null, permission: defaultPermission(runner) };
}

function readStored(): Record<string, RunnerConfig> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    return parsed as Record<string, RunnerConfig>;
  } catch {
    // Corrupt or unavailable storage is not worth failing a render over.
    return {};
  }
}

export interface RunnerConfigStore {
  /** The remembered config for a runner, filled in with that runner's defaults. */
  recall: (runner: string) => RunnerConfig;
  /** Record part of a runner's config. Empty values are ignored, not stored. */
  remember: (runner: string, patch: Partial<RunnerConfig>) => void;
}

export function useRunnerConfig(): RunnerConfigStore {
  const [configs, setConfigs] = useState<Record<string, RunnerConfig>>(readStored);
  // Read at call time so `recall` is a stable identity and never re-renders a
  // consumer just because another runner's config changed.
  const ref = useRef(configs);
  ref.current = configs;

  const recall = useCallback((runner: string): RunnerConfig => {
    const stored = ref.current[runner];
    if (!stored) return emptyConfig(runner);
    return { ...emptyConfig(runner), ...stored };
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

    setConfigs((current) => {
      const next = {
        ...current,
        [runner]: { ...emptyConfig(runner), ...current[runner], ...meaningful },
      };
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        // Storage full or blocked — the in-memory map still works this session.
      }
      return next;
    });
  }, []);

  return { recall, remember };
}
