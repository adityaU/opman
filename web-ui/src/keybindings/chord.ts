import type { ChordSeq, ChordStep, Platform } from "./types";

/**
 * Chord parsing and normalization.
 *
 * Two authoring styles share one syntax. Modifier chords are written
 * `ctrl+k ctrl+w` — space separates the steps of a sequence. Vim chords are
 * written as literal character runs, `<leader>sn` or `]s`, where each character
 * is its own step. A token is read as a modifier chord when it contains `+`,
 * and as a character run otherwise, so the two never need distinguishing by the
 * caller.
 */

const MODIFIERS = new Set(["ctrl", "shift", "alt", "meta", "cmd"]);

/** Names accepted inside `<>` and the normalized key they produce. */
const SPECIAL_KEYS: Readonly<Record<string, string>> = {
  leader: "<leader>",
  localleader: "<localleader>",
  space: "space",
  cr: "enter",
  enter: "enter",
  esc: "escape",
  escape: "escape",
  tab: "tab",
  bs: "backspace",
  backspace: "backspace",
  del: "delete",
  delete: "delete",
  up: "up",
  down: "down",
  left: "left",
  right: "right",
  home: "home",
  end: "end",
  pageup: "pageup",
  pagedown: "pagedown",
  lt: "<",
  gt: ">",
  bar: "|",
};

/** Browser `KeyboardEvent.key` values that need folding to our names. */
const EVENT_KEYS: Readonly<Record<string, string>> = {
  " ": "space",
  arrowup: "up",
  arrowdown: "down",
  arrowleft: "left",
  arrowright: "right",
  esc: "escape",
};

export class ChordParseError extends Error {}

/**
 * `mod` is the primary modifier — Command on macOS, Control everywhere else.
 * Authoring the base layer with it is what keeps that layer platform-neutral,
 * so a platform is a rewrite rule rather than a second copy of the keymap.
 */
export function applyPrimaryModifier(key: string, platform: Platform): string {
  return key.replace(/\bmod\+/gi, platform === "mac" ? "meta+" : "ctrl+");
}

const EMPTY_MODS = { ctrl: false, shift: false, alt: false, meta: false } as const;

function step(key: string, mods: Partial<ChordStep> = {}): ChordStep {
  return { ...EMPTY_MODS, ...mods, key };
}

function normalizeKeyName(raw: string): string {
  const lower = raw.toLowerCase();
  return EVENT_KEYS[lower] ?? lower;
}

/** Resolve a key written as `<cr>` / `<esc>`; plain names pass through. */
function resolveKeyToken(raw: string, token: string): string {
  if (!raw.startsWith("<")) return normalizeKeyName(raw);
  if (!raw.endsWith(">")) throw new ChordParseError(`unclosed "<" in "${token}"`);
  const name = raw.slice(1, -1).toLowerCase();
  const key = SPECIAL_KEYS[name];
  if (!key) throw new ChordParseError(`unknown key <${name}> in "${token}"`);
  return key;
}

function parseModifierChord(token: string): ChordStep {
  const parts = token.split("+").filter((p) => p.length > 0);
  if (parts.length === 0) throw new ChordParseError(`empty chord in "${token}"`);

  // A trailing literal `+` survives the filter above as a missing final part.
  const key = token.endsWith("+") ? "+" : parts[parts.length - 1];
  const mods = token.endsWith("+") ? parts : parts.slice(0, -1);

  let ctrl = false;
  let shift = false;
  let alt = false;
  let meta = false;
  for (const mod of mods) {
    const name = mod.toLowerCase();
    if (!MODIFIERS.has(name)) throw new ChordParseError(`unknown modifier "${mod}" in "${token}"`);
    if (name === "ctrl") ctrl = true;
    else if (name === "shift") shift = true;
    else if (name === "alt") alt = true;
    else meta = true;
  }
  return { ctrl, shift, alt, meta, key: resolveKeyToken(key, token) };
}

/** Read `<name>` at `i`, returning the normalized key and the index after `>`. */
function readAngleKey(token: string, i: number): { key: string; next: number } {
  const close = token.indexOf(">", i);
  if (close < 0) throw new ChordParseError(`unclosed "<" in "${token}"`);
  return { key: resolveKeyToken(token.slice(i, close + 1), token), next: close + 1 };
}

/** Multi-character key names that stand alone rather than as a character run. */
const NAMED_KEYS: ReadonlySet<string> = new Set([
  ...Object.values(SPECIAL_KEYS),
  ...Array.from({ length: 12 }, (_, i) => `f${i + 1}`),
]);

/**
 * Expand a character run such as `<leader>sn` or `]s` into one step per key.
 *
 * A token that is itself a key name is one step, not a run: `escape` is the
 * Escape key and not e-s-c-a-p-e, and `f1` is a function key and not f-then-1.
 */
function parseCharRun(token: string): ChordStep[] {
  const named = token.toLowerCase();
  if (NAMED_KEYS.has(named)) return [step(named)];

  const steps: ChordStep[] = [];
  let i = 0;
  while (i < token.length) {
    if (token[i] === "<") {
      const { key, next } = readAngleKey(token, i);
      steps.push(step(key));
      i = next;
      continue;
    }
    const char = token[i];
    // An upper-case letter in vim notation means the shifted key.
    if (char >= "A" && char <= "Z") steps.push(step(char.toLowerCase(), { shift: true }));
    else steps.push(step(char));
    i += 1;
  }
  if (steps.length === 0) throw new ChordParseError(`empty chord "${token}"`);
  return steps;
}

/** Parse an authored chord string into a sequence. Throws `ChordParseError`. */
export function parseChord(input: string): ChordSeq {
  const tokens = input.trim().split(/\s+/).filter((t) => t.length > 0);
  if (tokens.length === 0) throw new ChordParseError("empty chord");

  const steps: ChordStep[] = [];
  for (const token of tokens) {
    if (token.includes("+") && token !== "+") steps.push(parseModifierChord(token));
    else steps.push(...parseCharRun(token));
  }
  return steps;
}

/** Substitute `<leader>` / `<localleader>` for the configured keys. */
export function expandLeaders(seq: ChordSeq, leader: ChordSeq, localLeader: ChordSeq): ChordSeq {
  if (!seq.some((s) => s.key === "<leader>" || s.key === "<localleader>")) return seq;
  const out: ChordStep[] = [];
  for (const s of seq) {
    if (s.key === "<leader>") out.push(...leader);
    else if (s.key === "<localleader>") out.push(...localLeader);
    else out.push(s);
  }
  return out;
}

export function formatStep(s: ChordStep): string {
  const parts: string[] = [];
  if (s.ctrl) parts.push("ctrl");
  if (s.shift) parts.push("shift");
  if (s.alt) parts.push("alt");
  if (s.meta) parts.push("meta");
  parts.push(s.key);
  return parts.join("+");
}

/** Stable text form used as the identity of a binding. */
export function formatChord(seq: ChordSeq): string {
  return seq.map(formatStep).join(" ");
}

export function stepsEqual(a: ChordStep, b: ChordStep): boolean {
  return (
    a.key === b.key && a.ctrl === b.ctrl && a.shift === b.shift && a.alt === b.alt && a.meta === b.meta
  );
}

/** True when `prefix` is a strict prefix of `seq`. */
export function isPrefixOf(prefix: ChordSeq, seq: ChordSeq): boolean {
  if (prefix.length >= seq.length) return false;
  return prefix.every((s, i) => stepsEqual(s, seq[i]));
}

/**
 * Read a live keydown into a step.
 *
 * `cmd` is authored per-platform rather than folded here, so a mac binding that
 * means Control really means Control. The one fold is Shift: it is recorded
 * from the event, and printable keys are lower-cased so `shift+p` and `P` are
 * the same step.
 */
export function stepFromEvent(e: KeyboardEvent): ChordStep {
  const key = normalizeKeyName(e.key);
  return {
    ctrl: e.ctrlKey,
    shift: e.shiftKey,
    alt: e.altKey,
    meta: e.metaKey,
    key: key.length === 1 ? key.toLowerCase() : key,
  };
}

const SYMBOLS: Readonly<Record<string, string>> = {
  ctrl: "⌃",
  shift: "⇧",
  alt: "⌥",
  meta: "⌘",
};

/** Human-readable form for the cheatsheet and the keybindings view. */
export function displayStep(s: ChordStep, platform: Platform): string {
  const mac = platform === "mac";
  const parts: string[] = [];
  if (s.ctrl) parts.push(mac ? SYMBOLS.ctrl : "Ctrl");
  if (s.shift) parts.push(mac ? SYMBOLS.shift : "Shift");
  if (s.alt) parts.push(mac ? SYMBOLS.alt : "Alt");
  if (s.meta) parts.push(mac ? SYMBOLS.meta : "Meta");
  parts.push(s.key.length === 1 ? s.key.toUpperCase() : s.key);
  return mac ? parts.join("") : parts.join("+");
}

export function displayChord(seq: ChordSeq, platform: Platform): string {
  return seq.map((s) => displayStep(s, platform)).join(" ");
}
