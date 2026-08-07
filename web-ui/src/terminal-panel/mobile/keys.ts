/**
 * The keys a phone keyboard does not have.
 *
 * A soft keyboard offers letters, digits and Enter. Everything a terminal
 * actually needs to be usable — Esc, Tab, Ctrl, arrows, page motion — has no
 * key to press, so those are provided here as explicit escape sequences.
 */

export interface KeySpec {
  /** Stable id, also the aria-label stem. */
  id: string;
  /** What the button reads as. Kept to 1–3 glyphs so the row never wraps. */
  label: string;
  /** Bytes sent to the pty. */
  seq: string;
  /** Longer name for assistive tech, when the label is a glyph. */
  title: string;
}

/** Navigation cluster: the arrows, as their own group so they can be laid out
 *  as a d-pad-ish row that is hard to mis-tap. */
export const ARROW_KEYS: KeySpec[] = [
  { id: "left", label: "←", seq: "\x1b[D", title: "Left arrow" },
  { id: "up", label: "↑", seq: "\x1b[A", title: "Up arrow" },
  { id: "down", label: "↓", seq: "\x1b[B", title: "Down arrow" },
  { id: "right", label: "→", seq: "\x1b[C", title: "Right arrow" },
];

/** Always-visible keys — the ones needed to escape a mode or complete a path. */
export const PRIMARY_KEYS: KeySpec[] = [
  { id: "esc", label: "esc", seq: "\x1b", title: "Escape" },
  { id: "tab", label: "tab", seq: "\t", title: "Tab — complete" },
];

/**
 * Second row, revealed on demand: shell control codes, page motion and the
 * punctuation that soft keyboards bury two layers deep.
 */
export const EXTRA_KEYS: KeySpec[] = [
  { id: "ctrl-c", label: "^C", seq: "\x03", title: "Interrupt (Ctrl+C)" },
  { id: "ctrl-d", label: "^D", seq: "\x04", title: "End of input (Ctrl+D)" },
  { id: "ctrl-z", label: "^Z", seq: "\x1a", title: "Suspend (Ctrl+Z)" },
  { id: "ctrl-l", label: "^L", seq: "\x0c", title: "Clear screen (Ctrl+L)" },
  { id: "ctrl-r", label: "^R", seq: "\x12", title: "Reverse search (Ctrl+R)" },
  { id: "ctrl-a", label: "^A", seq: "\x01", title: "Start of line (Ctrl+A)" },
  { id: "ctrl-e", label: "^E", seq: "\x05", title: "End of line (Ctrl+E)" },
  { id: "ctrl-u", label: "^U", seq: "\x15", title: "Clear line (Ctrl+U)" },
  { id: "home", label: "home", seq: "\x1b[H", title: "Home" },
  { id: "end", label: "end", seq: "\x1b[F", title: "End" },
  { id: "pgup", label: "pgup", seq: "\x1b[5~", title: "Page up" },
  { id: "pgdn", label: "pgdn", seq: "\x1b[6~", title: "Page down" },
  { id: "pipe", label: "|", seq: "|", title: "Pipe" },
  { id: "slash", label: "/", seq: "/", title: "Slash" },
  { id: "dash", label: "-", seq: "-", title: "Dash" },
  { id: "tilde", label: "~", seq: "~", title: "Tilde" },
];

/** Modifier state. `armed` fires once, `locked` stays until tapped off. */
export type ModifierState = "off" | "armed" | "locked";

/** Advance a modifier on tap: off → armed → locked → off. */
export function nextModifierState(state: ModifierState): ModifierState {
  if (state === "off") return "armed";
  if (state === "armed") return "locked";
  return "off";
}

/**
 * Apply held modifiers to keyboard input.
 *
 * Ctrl maps a letter to its control code the way a real Ctrl does (mask 0x1f);
 * Alt sends ESC first, which is how terminals have always encoded Meta. Input
 * the modifier cannot describe is passed through untouched rather than dropped.
 */
export function applyModifiers(
  data: string,
  ctrl: boolean,
  alt: boolean,
): string {
  let out = data;
  if (ctrl && out.length === 1) {
    const code = out.toLowerCase().charCodeAt(0);
    // @ through _ plus lowercase letters are the range Ctrl can encode.
    if (code >= 0x40 && code <= 0x7f) out = String.fromCharCode(code & 0x1f);
    else if (code === 0x20) out = "\x00";
  }
  if (alt) out = `\x1b${out}`;
  return out;
}
