import { useCallback, useMemo } from "react";
import { displayChord } from "./chord";
import { useOptionalKeymapContext } from "./KeymapContext";
import type { Keymap } from "./matcher";
import type { CommandId, Host, Mode, ResolvedBinding } from "./types";

/**
 * One answer to "what key runs this right now", for every surface that shows a
 * shortcut beside a control.
 *
 * Before this, roughly thirty controls carried a hand-written chord in a
 * `title` string — all of them macOS-flavoured and all of them normal-mode, so
 * a Linux user in vim mode was told to press keys that did nothing. The chord
 * belongs to the composed keymap, which already knows the platform, the browser
 * quirks and the user's own `keybindings.json`; nothing should restate it.
 */

/**
 * Which of a command's chords to advertise.
 *
 * A command reachable both ways — `Ctrl+\`` and `<leader>ot` for the terminal —
 * should be shown in the idiom of the mode the user chose, so a binding
 * authored for this mode wins over one that applies to both. Within that, an
 * unscoped chord beats a `when`-scoped one: the scoped chord is the one that
 * only works somewhere, and a control's label should name the key that always
 * does.
 */
export function preferredBinding(
  bindings: readonly ResolvedBinding[],
  mode: Mode,
): ResolvedBinding | undefined {
  if (bindings.length <= 1) return bindings[0];
  const native = bindings.filter((binding) => binding.mode === mode);
  const pool = native.length > 0 ? native : bindings;
  return pool.find((binding) => !binding.when) ?? pool[0];
}

/** The display chord for a command, or `undefined` when nothing is bound. */
export function chordLabel(
  keymap: Keymap,
  host: Host,
  mode: Mode,
  command: CommandId | undefined,
): string | undefined {
  if (!command) return undefined;
  const binding = preferredBinding(keymap.chordsFor(command), mode);
  return binding && displayChord(binding.seq, host.platform, mode);
}

/**
 * A labeller for a list of controls.
 *
 * Menus build their rows per render and cannot call a hook per row, so they take
 * this function instead. It is memoized on the keymap, so a menu re-renders
 * without re-resolving anything until a binding actually changes.
 */
export function useChordLabeller(): (command: CommandId | undefined) => string | undefined {
  const keymap = useOptionalKeymapContext();
  return useCallback(
    (command) => (keymap ? chordLabel(keymap.keymap, keymap.host, keymap.mode, command) : undefined),
    [keymap],
  );
}

/**
 * The display chord for one command.
 *
 * Safe outside a `KeymapProvider` — the login screen and the mobile shell mount
 * without one, and a missing shortcut is the honest rendering there anyway.
 */
export function useChord(command: CommandId | undefined): string | undefined {
  const keymap = useOptionalKeymapContext();
  return useMemo(
    () => (keymap ? chordLabel(keymap.keymap, keymap.host, keymap.mode, command) : undefined),
    [keymap, command],
  );
}
