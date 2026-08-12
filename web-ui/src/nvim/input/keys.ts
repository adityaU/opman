import { KEY_TABLE, KEYPAD_TABLE, MODIFIER_KEYS } from "./keyTable";

export interface ReleaseFocusChord {
  readonly key: string;
  readonly code?: string;
  readonly shiftKey?: boolean;
  readonly ctrlKey?: boolean;
  readonly altKey?: boolean;
  readonly metaKey?: boolean;
}

export const DEFAULT_RELEASE_FOCUS_CHORD: ReleaseFocusChord = Object.freeze({ key: "Escape", ctrlKey: true, shiftKey: true });

export interface KeyEncodingOptions {
  /** On macOS, treat Option-composed characters as Meta instead of Alt. */
  readonly macAltIsMeta?: boolean;
}

export interface KeyDispatchOptions extends KeyEncodingOptions {
  readonly onInput: (keys: string) => void;
  readonly releaseFocus?: ReleaseFocusChord | null;
  readonly onReleaseFocus?: () => void;
}

const CTRL_CODE_BASE: Readonly<Record<string, string>> = Object.freeze({
  Digit0: "0", Digit1: "1", Digit2: "2", Digit3: "3", Digit4: "4",
  Digit5: "5", Digit6: "6", Digit7: "7", Digit8: "8", Digit9: "9",
  Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]",
  Backslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".",
  Slash: "/", Backquote: "`",
});

const LETTER_CODE_BASE: Readonly<Record<string, string>> = Object.freeze({
  KeyA: "a", KeyB: "b", KeyC: "c", KeyD: "d", KeyE: "e", KeyF: "f",
  KeyG: "g", KeyH: "h", KeyI: "i", KeyJ: "j", KeyK: "k", KeyL: "l",
  KeyM: "m", KeyN: "n", KeyO: "o", KeyP: "p", KeyQ: "q", KeyR: "r",
  KeyS: "s", KeyT: "t", KeyU: "u", KeyV: "v", KeyW: "w", KeyX: "x",
  KeyY: "y", KeyZ: "z",
});

function isComposing(event: KeyboardEvent): boolean {
  return event.isComposing === true || event.keyCode === 229;
}

function altGraphActive(event: KeyboardEvent): boolean {
  return typeof event.getModifierState === "function" && event.getModifierState("AltGraph");
}

function codeBase(code: string): string | undefined {
  return CTRL_CODE_BASE[code] ?? LETTER_CODE_BASE[code];
}

function isMacOptionComposition(event: KeyboardEvent, base: string | undefined): boolean {
  return event.altKey && base !== undefined && event.key.length === 1 && event.key !== base;
}

function literalToken(value: string): string {
  if (value === "<") return "lt";
  if (value === "\\") return "Bslash";
  return value;
}

function wrapKey(
  key: string,
  named: boolean,
  event: KeyboardEvent,
  altGraph: boolean,
  options: KeyEncodingOptions,
): string {
  const codeKey = codeBase(event.code);
  const optionComposition = !altGraph && isMacOptionComposition(event, codeKey);
  const optionAsMeta = options.macAltIsMeta === true && optionComposition;
  const shift = event.shiftKey && named;
  const ctrl = event.ctrlKey && !altGraph;
  const alt = event.altKey && !altGraph && !optionAsMeta;
  const meta = event.metaKey || optionAsMeta;
  let modifiers = "";
  if (shift) modifiers += "S-";
  if (ctrl) modifiers += "C-";
  if (alt) modifiers += "A-";
  if (meta) modifiers += "D-";
  if (modifiers.length === 0) return named ? `<${key}>` : key === "<" ? "<lt>" : key;
  return `<${modifiers}${literalToken(key)}>`;
}

/** Encode one KeyboardEvent as Neovim key notation, or null when it is not consumed. */
export function encodeKey(event: KeyboardEvent, options: KeyEncodingOptions = {}): string | null {
  if (isComposing(event) || MODIFIER_KEYS.has(event.key)) return null;

  const altGraph = altGraphActive(event);
  const keypad = KEYPAD_TABLE[event.code];
  const codeKey = codeBase(event.code);
  const ctrlFallback = event.ctrlKey && !altGraph ? CTRL_CODE_BASE[event.code] : undefined;
  const macOptionBase = !altGraph && isMacOptionComposition(event, codeKey) ? codeKey : undefined;
  const named = keypad !== undefined || event.key.length !== 1;
  const printable = event.key.length === 1 && event.shiftKey && /^[a-z]$/.test(event.key)
    ? event.key.toUpperCase()
    : event.key;
  const key = keypad ?? ctrlFallback ?? macOptionBase ?? (printable.length === 1 ? printable : KEY_TABLE[event.key]);
  if (key === undefined || key.length === 0) return null;
  return wrapKey(key, named || keypad !== undefined || KEY_TABLE[event.key] !== undefined, event, altGraph, options);
}

/** Compatibility name for callers migrating from the deleted panel-local encoder. */
export const encodeNeovimKey = encodeKey;

function matchesChord(event: KeyboardEvent, chord: ReleaseFocusChord): boolean {
  return event.key === chord.key
    && (chord.code === undefined || event.code === chord.code)
    && (chord.shiftKey === undefined || event.shiftKey === chord.shiftKey)
    && (chord.ctrlKey === undefined || event.ctrlKey === chord.ctrlKey)
    && (chord.altKey === undefined || event.altKey === chord.altKey)
    && (chord.metaKey === undefined || event.metaKey === chord.metaKey);
}

export function isReleaseFocusKey(
  event: KeyboardEvent,
  chord: ReleaseFocusChord = DEFAULT_RELEASE_FOCUS_CHORD,
): boolean {
  return !isComposing(event) && matchesChord(event, chord);
}

/** Dispatch a key and prevent its browser default when U9 consumed it. */
export function dispatchKeyDown(event: KeyboardEvent, options: KeyDispatchOptions): boolean {
  const releaseFocus = options.releaseFocus === undefined ? DEFAULT_RELEASE_FOCUS_CHORD : options.releaseFocus;
  if (releaseFocus !== null && !isComposing(event) && matchesChord(event, releaseFocus)) {
    event.preventDefault();
    options.onReleaseFocus?.();
    return true;
  }
  const keys = encodeKey(event, options);
  if (keys === null) return false;
  event.preventDefault();
  options.onInput(keys);
  return true;
}

export const handleKeyDown = dispatchKeyDown;
