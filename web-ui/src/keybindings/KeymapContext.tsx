import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { configLayer, DEFAULT_CONFIG, userLayer } from "./config";
import type { KeybindingsConfig } from "./config";
import { detectHost } from "./host";
import { builtInLayers } from "./layers";
import { Keymap } from "./matcher";
import { resolve } from "./resolve";
import type { BindingSpec, CommandId, Conflict, Host, Mode } from "./types";
import type { WhenContext } from "./when";

/**
 * The keymap context.
 *
 * Holds the composed keymap, the context keys surfaces publish, and the
 * registry of command handlers. Nothing here listens for keys — that is
 * `useKeymapListener`, so the composition can be rendered and tested without a
 * live document.
 */

export type CommandHandler = () => void;

export interface KeymapValue {
  readonly keymap: Keymap;
  readonly host: Host;
  readonly mode: Mode;
  readonly config: KeybindingsConfig;
  readonly rejected: readonly Conflict[];
  /** Context keys published by surfaces, read by `when` clauses. */
  readonly context: WhenContext;
  setContext: (patch: WhenContext) => void;
  registerCommand: (id: CommandId, handler: CommandHandler) => () => void;
  runCommand: (id: CommandId) => boolean;
  /** Command ids with a handler right now — used to grey out palette rows. */
  isRunnable: (id: CommandId) => boolean;
}

const KeymapContext = createContext<KeymapValue | undefined>(undefined);

export interface KeymapProviderProps {
  readonly children: ReactNode;
  readonly config?: KeybindingsConfig;
  /** Per-device overrides from the keybindings view. */
  readonly overrides?: readonly BindingSpec[];
  /** Injected in tests; detected from the browser otherwise. */
  readonly host?: Host;
}

export function KeymapProvider({
  children,
  config = DEFAULT_CONFIG,
  overrides = [],
  host,
}: KeymapProviderProps) {
  const [context, setContextState] = useState<WhenContext>({});
  const handlers = useRef(new Map<CommandId, CommandHandler>());
  const resolvedHost = useMemo(() => host ?? detectHost(), [host]);

  const { keymap, rejected } = useMemo(() => {
    const layers = [...builtInLayers(), configLayer(config), userLayer(overrides)];
    const result = resolve(layers, {
      host: resolvedHost,
      mode: config.mode,
      leader: config.leader,
      localLeader: config.localLeader,
    });
    return { keymap: new Keymap(result.bindings), rejected: result.rejected };
  }, [config, overrides, resolvedHost]);

  /**
   * These three are referentially stable for the life of the provider.
   *
   * That is load-bearing, not tidiness: `useWhenContext` cleans up by clearing
   * the keys it published, so a `setContext` that changed identity whenever the
   * context changed would re-run that effect, clear, re-publish, and loop
   * forever. Handlers live in a ref for the same reason.
   */
  const setContext = useCallback((patch: WhenContext) => {
    setContextState((previous) => {
      const changed = Object.entries(patch).some(([key, next]) => previous[key] !== next);
      return changed ? { ...previous, ...patch } : previous;
    });
  }, []);

  const registerCommand = useCallback((id: CommandId, handler: CommandHandler) => {
    handlers.current.set(id, handler);
    return () => {
      // Only clear the entry if it is still ours: a remount can register the
      // replacement before the old surface's cleanup runs.
      if (handlers.current.get(id) === handler) handlers.current.delete(id);
    };
  }, []);

  const runCommand = useCallback((id: CommandId) => {
    const handler = handlers.current.get(id);
    if (!handler) return false;
    handler();
    return true;
  }, []);

  const value = useMemo<KeymapValue>(
    () => ({
      keymap,
      host: resolvedHost,
      mode: config.mode,
      config,
      rejected,
      context,
      setContext,
      registerCommand,
      runCommand,
      isRunnable: (id) => handlers.current.has(id),
    }),
    [keymap, resolvedHost, config, rejected, context, setContext, registerCommand, runCommand],
  );

  return <KeymapContext.Provider value={value}>{children}</KeymapContext.Provider>;
}

export function useKeymapContext(): KeymapValue {
  const value = useContext(KeymapContext);
  if (!value) throw new Error("useKeymapContext must be used inside a KeymapProvider");
  return value;
}

/** Read the context without requiring a provider — for optional call sites. */
export function useOptionalKeymapContext(): KeymapValue | undefined {
  return useContext(KeymapContext);
}
