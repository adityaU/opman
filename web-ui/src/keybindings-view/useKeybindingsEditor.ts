import { useCallback, useEffect, useState } from "react";
import {
  loadKeybindingsOrDefault,
  publishKeybindings,
  saveKeybindings,
} from "../api/keybindings";
import type { ConfigDiagnostic, KeybindingsConfig } from "../keybindings/config";
import { DEFAULT_CONFIG } from "../keybindings/config";
import type { BindingSpec, CommandId, Mode } from "../keybindings/types";

/**
 * Editing state for the keybindings view.
 *
 * Every edit is expressed as an entry in `config.bindings` and written to
 * `keybindings.json`. There is no second store: what the view shows and what
 * the file says are the same thing, so the two can never drift.
 */

export interface KeybindingsEditor {
  readonly config: KeybindingsConfig;
  readonly diagnostics: readonly ConfigDiagnostic[];
  readonly path: string | null;
  readonly loading: boolean;
  readonly saving: boolean;
  readonly error: string | undefined;
  setMode: (mode: Mode) => void;
  setLeader: (leader: string) => void;
  setWhichKey: (patch: Partial<KeybindingsConfig["whichKey"]>) => void;
  rebind: (command: CommandId, chord: string, previous?: string, when?: string) => void;
  unbind: (command: CommandId, chord: string) => void;
  reset: (command: CommandId) => void;
  resetAll: () => void;
  reload: () => void;
}

/** Drop every config entry that concerns a command, add or remove alike. */
function withoutCommand(
  bindings: readonly BindingSpec[],
  command: CommandId,
): BindingSpec[] {
  return bindings.filter((entry) => entry.command !== command && entry.command !== `-${command}`);
}

export function useKeybindingsEditor(): KeybindingsEditor {
  const [config, setConfig] = useState<KeybindingsConfig>(DEFAULT_CONFIG);
  const [diagnostics, setDiagnostics] = useState<readonly ConfigDiagnostic[]>([]);
  const [path, setPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();

  const reload = useCallback(() => {
    let cancelled = false;
    setLoading(true);
    loadKeybindingsOrDefault().then((response) => {
      if (cancelled) return;
      setConfig(response.config);
      setDiagnostics(response.diagnostics);
      setPath(response.path);
      setLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => reload(), [reload]);

  /**
   * Persist optimistically: the local config updates first so the table never
   * lags a keystroke behind, and a failed write surfaces as an error rather
   * than silently reverting an edit the user watched land.
   *
   * The publish is what keeps "no second store" true. `KeymapRoot` holds the
   * copy the live keymap is composed from and fetches it once at mount, so
   * without this an edit reaches the file and the table but not the keyboard —
   * and a mode switch, whose active state this view reads back from that
   * app-level copy, looks like it did not take until the page is reloaded.
   */
  const commit = useCallback((next: KeybindingsConfig) => {
    setConfig(next);
    publishKeybindings(next);
    setSaving(true);
    setError(undefined);
    saveKeybindings(next)
      .then((response) => {
        setDiagnostics(response.diagnostics);
        setPath(response.path);
      })
      .catch((cause: unknown) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      })
      .finally(() => setSaving(false));
  }, []);

  const setMode = useCallback(
    (mode: Mode) => commit({ ...config, mode }),
    [commit, config],
  );

  const setLeader = useCallback(
    (leader: string) => commit({ ...config, leader }),
    [commit, config],
  );

  const setWhichKey = useCallback(
    (patch: Partial<KeybindingsConfig["whichKey"]>) =>
      commit({ ...config, whichKey: { ...config.whichKey, ...patch } }),
    [commit, config],
  );

  /**
   * Rebinding removes the old chord explicitly rather than relying on
   * supersession: config entries add, so without the removal the user would end
   * up with both chords live and no way to tell which one they meant.
   */
  const rebind = useCallback(
    (command: CommandId, chord: string, previous?: string, when?: string) => {
      const bindings = [...config.bindings];
      if (previous) bindings.push({ key: previous, command: `-${command}` });
      bindings.push({ key: chord, command, when });
      commit({ ...config, bindings });
    },
    [commit, config],
  );

  const unbind = useCallback(
    (command: CommandId, chord: string) =>
      commit({ ...config, bindings: [...config.bindings, { key: chord, command: `-${command}` }] }),
    [commit, config],
  );

  const reset = useCallback(
    (command: CommandId) =>
      commit({ ...config, bindings: withoutCommand(config.bindings, command) }),
    [commit, config],
  );

  const resetAll = useCallback(() => commit({ ...config, bindings: [] }), [commit, config]);

  return {
    config,
    diagnostics,
    path,
    loading,
    saving,
    error,
    setMode,
    setLeader,
    setWhichKey,
    rebind,
    unbind,
    reset,
    resetAll,
    reload,
  };
}
