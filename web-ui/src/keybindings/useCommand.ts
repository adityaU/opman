import { useEffect, useRef } from "react";
import { useKeymapContext } from "./KeymapContext";
import type { CommandHandler } from "./KeymapContext";
import type { CommandId } from "./types";
import type { WhenContext } from "./when";

/**
 * How a surface joins the keymap.
 *
 * A surface registers what it can *do* and publishes what is *true* about it.
 * It never names a key — that belongs to the keymap layers, which is what makes
 * a binding configurable without touching the surface that implements it.
 */

/** Register a handler for one command for as long as the component is mounted. */
export function useCommand(id: CommandId, handler: CommandHandler, enabled = true): void {
  const { registerCommand } = useKeymapContext();
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (!enabled) return undefined;
    return registerCommand(id, () => handlerRef.current());
  }, [id, enabled, registerCommand]);
}

/**
 * Register several commands at once.
 *
 * The map is read through a ref, so a caller may pass a fresh object literal on
 * every render without re-registering — the common case for a surface whose
 * handlers close over current state.
 */
export function useCommands(handlers: Readonly<Record<CommandId, CommandHandler>>): void {
  const { registerCommand } = useKeymapContext();
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  const ids = Object.keys(handlers).sort().join(",");

  useEffect(() => {
    const unregister = ids
      .split(",")
      .filter((id) => id.length > 0)
      .map((id) => registerCommand(id, () => handlersRef.current[id]?.()));
    return () => unregister.forEach((fn) => fn());
  }, [ids, registerCommand]);
}

/**
 * Publish context keys for `when` clauses.
 *
 * Values are cleared on unmount so a closed panel cannot leave `focus==git`
 * true and steal every git binding from the rest of the app.
 */
export function useWhenContext(patch: WhenContext): void {
  const { setContext } = useKeymapContext();
  const serialized = JSON.stringify(patch);

  useEffect(() => {
    const parsed: WhenContext = JSON.parse(serialized);
    setContext(parsed);
    return () => {
      const cleared = Object.fromEntries(Object.keys(parsed).map((key) => [key, undefined]));
      setContext(cleared);
    };
  }, [serialized, setContext]);
}
