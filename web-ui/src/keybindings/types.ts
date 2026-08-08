/**
 * Core types for the layered keybinding system.
 *
 * The keymap is data, never code: layers of `BindingSpec` are composed by
 * `resolve()` into a flat `ResolvedBinding[]` that the matcher walks. Adding a
 * platform or a target means adding a layer, not editing the others.
 */

export type Mode = "normal" | "vim";
export type Platform = "mac" | "win" | "linux";
export type Target = "web" | "desktop";
export type Browser = "chrome" | "firefox" | "safari" | "other";

/** Where the app is running. Drives which layers apply. */
export interface Host {
  readonly platform: Platform;
  readonly target: Target;
  readonly browser: Browser;
}

/** One key press: modifiers plus a normalized key name. */
export interface ChordStep {
  readonly ctrl: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
  readonly meta: boolean;
  readonly key: string;
}

/** A chord sequence — one step for a single press, more for `ctrl+k ctrl+w`. */
export type ChordSeq = readonly ChordStep[];

export type CommandId = string;

/**
 * How a command is reachable by typing `/name` in the composer.
 *
 * `where` is the whole point of the field. `opman` commands are opman's own — a panel, a
 * modal, a page — and are listed and run by the client. Commands the agent executes are
 * never listed here at all: the runner reports those, because only the runner knows which
 * ones it has. `where: "runner"` marks the few that opman *sends* on behalf of a keybinding,
 * so a chord for "compact" reaches whichever agent the session is on.
 */
export interface SlashSpec {
  /** The name typed after the slash, without it. */
  readonly name: string;
  readonly where: "opman" | "runner";
}

/**
 * A command is the unit everything else is generated from: the palette, the
 * cheatsheet, the which-key hints and the slash popover all read this registry.
 */
export interface CommandDef {
  readonly id: CommandId;
  readonly title: string;
  readonly category: string;
  /** Context expression gating availability, e.g. `sessionActive && focus==editor`. */
  readonly when?: string;
  /** Short lower-case label for which-key. Falls back to `title`. */
  readonly label?: string;
  /** Palette-only commands are reachable but never bound by default. */
  readonly paletteOnly?: boolean;
  /** The `/name` that invokes this command from the composer, if it has one. */
  readonly slash?: SlashSpec;
}

/**
 * A binding as authored in a layer or in `keybindings.json`.
 *
 * `command` prefixed with `-` removes an earlier binding rather than adding one,
 * matching the VSCode convention. `-*` clears everything bound so far.
 */
export interface BindingSpec {
  readonly key: string;
  readonly command: CommandId;
  readonly when?: string;
  /** Defaults to applying in both modes. */
  readonly mode?: Mode;
  readonly platform?: Platform;
  readonly target?: Target;
  readonly browser?: Browser;
  /** Which-key namespace, e.g. `git`. Vim bindings only. */
  readonly group?: string;
  /** Which-key leaf label. Overrides the command's own label. */
  readonly label?: string;
}

/** A binding after layer composition, with its chord parsed and normalized. */
export interface ResolvedBinding {
  readonly seq: ChordSeq;
  /** Normalized text form, e.g. `ctrl+k ctrl+w`. Stable and comparable. */
  readonly id: string;
  readonly command: CommandId;
  readonly when?: string;
  readonly mode?: Mode;
  readonly group?: string;
  readonly label?: string;
  /** Which layer contributed it — drives the "Default / Config / User" badge. */
  readonly source: BindingSource;
}

export type BindingSource = "base" | "platform" | "target" | "host" | "config" | "user";

export interface Layer {
  readonly source: BindingSource;
  readonly bindings: readonly BindingSpec[];
}

/** A chord the host operating system or browser takes before the page sees it. */
export interface ReservedChord {
  readonly id: string;
  readonly owner: string;
}

export type ConflictKind =
  | "duplicate"
  | "prefix-is-command"
  | "reserved"
  | "unknown-command"
  | "malformed";

export interface Conflict {
  readonly kind: ConflictKind;
  readonly chord: string;
  readonly detail: string;
  readonly commands: readonly CommandId[];
}
