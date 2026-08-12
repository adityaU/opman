import {
  DEFAULT_RELEASE_FOCUS_CHORD,
  dispatchKeyDown,
  isReleaseFocusKey,
  type ReleaseFocusChord,
} from "../input/keys";
import type { ModeShort } from "./wire";

export type NvimMode = ModeShort;

function isInsert(mode: NvimMode): boolean {
  return mode === "insert";
}

function isClipboardKey(event: KeyboardEvent, mode: NvimMode): boolean {
  if (event.altKey) return false;
  const key = event.key.toLowerCase();
  if (!(key === "c" || key === "v" || key === "x" || key === "a")) return false;
  return event.metaKey || (isInsert(mode) && event.ctrlKey);
}

/**
 * Insert mode leaves printable typing in CodeMirror for latency. Once the app
 * keymap stands aside, modifier chords such as `<C-w>`, `<C-r>`, and `<C-o>`
 * must still reach Neovim instead of falling through to nothing.
 */
function routesInInsertMode(event: KeyboardEvent, releaseFocus: ReleaseFocusChord): boolean {
  return event.key === "Escape" || isReleaseFocusKey(event, releaseFocus)
    || event.ctrlKey || event.altKey || event.metaKey;
}

/** Route one DOM key without duplicating Neovim's validated encoder. */
export function routeNvimKey(
  event: KeyboardEvent,
  mode: NvimMode,
  onInput: (keys: string) => void,
  onReleaseFocus: () => void,
  releaseFocus: ReleaseFocusChord = DEFAULT_RELEASE_FOCUS_CHORD,
): boolean {
  if (isClipboardKey(event, mode)) return false;
  if (isInsert(mode) && !routesInInsertMode(event, releaseFocus)) return false;
  return dispatchKeyDown(event, { onInput, onReleaseFocus, releaseFocus });
}
