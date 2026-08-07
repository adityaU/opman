import type { BindingSpec, Layer, Mode } from "./types";

/**
 * The user-authored config layer.
 *
 * Shape mirrors `~/.config/opman/keybindings.json`. The backend guarantees the
 * file is valid JSON of roughly this shape; everything below is the second half
 * of that contract — the parts only the web UI can check, because only it knows
 * which command ids and chord syntax exist.
 */

export interface WhichKeyConfig {
  readonly enabled: boolean;
  readonly delayMs: number;
  readonly sortBy: "group" | "key" | "label";
}

export interface KeybindingsConfig {
  readonly mode: Mode;
  readonly leader: string;
  readonly localLeader: string;
  readonly chordTimeoutMs: number;
  readonly whichKey: WhichKeyConfig;
  readonly bindings: readonly BindingSpec[];
}

export interface ConfigDiagnostic {
  readonly message: string;
  readonly line?: number;
  readonly column?: number;
}

export interface KeybindingsResponse {
  readonly config: KeybindingsConfig;
  readonly diagnostics: readonly ConfigDiagnostic[];
  readonly path: string | null;
}

export const DEFAULT_CONFIG: KeybindingsConfig = {
  mode: "normal",
  leader: "<space>",
  localLeader: ",",
  chordTimeoutMs: 1500,
  whichKey: { enabled: true, delayMs: 400, sortBy: "group" },
  bindings: [],
};

const MODES: ReadonlySet<string> = new Set(["normal", "vim"]);
const PLATFORMS: ReadonlySet<string> = new Set(["mac", "win", "linux"]);
const TARGETS: ReadonlySet<string> = new Set(["web", "desktop"]);
const BROWSERS: ReadonlySet<string> = new Set(["chrome", "firefox", "safari", "other"]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function optionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/** Keep an enum-valued field only when it is one of the values we know. */
function enumField<T extends string>(
  value: unknown,
  allowed: ReadonlySet<string>,
): T | undefined {
  const text = optionalString(value);
  return text && allowed.has(text) ? (text as T) : undefined;
}

/**
 * Read one authored binding, dropping anything malformed with a diagnostic.
 *
 * A single bad entry must never cost the user the rest of the file, so this
 * returns `undefined` and reports rather than throwing.
 */
function parseBinding(
  raw: unknown,
  index: number,
  report: (message: string) => void,
): BindingSpec | undefined {
  if (!isRecord(raw)) {
    report(`bindings[${index}] is not an object`);
    return undefined;
  }

  const key = optionalString(raw.key);
  const command = optionalString(raw.command);
  if (!key) {
    report(`bindings[${index}] is missing "key"`);
    return undefined;
  }
  if (!command) {
    report(`bindings[${index}] is missing "command"`);
    return undefined;
  }

  return {
    key,
    command,
    when: optionalString(raw.when),
    mode: enumField<Mode>(raw.mode, MODES),
    platform: enumField(raw.platform, PLATFORMS),
    target: enumField(raw.target, TARGETS),
    browser: enumField(raw.browser, BROWSERS),
    group: optionalString(raw.group),
    label: optionalString(raw.label),
  };
}

export interface ParsedConfig {
  readonly config: KeybindingsConfig;
  readonly diagnostics: readonly ConfigDiagnostic[];
}

/** Normalize a config from the backend, reporting whatever had to be dropped. */
export function parseConfig(raw: unknown): ParsedConfig {
  const diagnostics: ConfigDiagnostic[] = [];
  const report = (message: string) => diagnostics.push({ message });

  if (!isRecord(raw)) return { config: DEFAULT_CONFIG, diagnostics };

  const rawBindings = Array.isArray(raw.bindings) ? raw.bindings : [];
  if (raw.bindings !== undefined && !Array.isArray(raw.bindings)) {
    report('"bindings" is not an array');
  }

  const bindings = rawBindings
    .map((entry, index) => parseBinding(entry, index, report))
    .filter((entry): entry is BindingSpec => entry !== undefined);

  const whichKey = isRecord(raw.whichKey) ? raw.whichKey : {};

  return {
    config: {
      mode: enumField<Mode>(raw.mode, MODES) ?? DEFAULT_CONFIG.mode,
      leader: optionalString(raw.leader) ?? DEFAULT_CONFIG.leader,
      localLeader: optionalString(raw.localLeader) ?? DEFAULT_CONFIG.localLeader,
      chordTimeoutMs:
        typeof raw.chordTimeoutMs === "number" && raw.chordTimeoutMs > 0
          ? raw.chordTimeoutMs
          : DEFAULT_CONFIG.chordTimeoutMs,
      whichKey: {
        enabled: whichKey.enabled !== false,
        delayMs:
          typeof whichKey.delayMs === "number" && whichKey.delayMs >= 0
            ? whichKey.delayMs
            : DEFAULT_CONFIG.whichKey.delayMs,
        sortBy:
          enumField<WhichKeyConfig["sortBy"]>(
            whichKey.sortBy,
            new Set(["group", "key", "label"]),
          ) ?? DEFAULT_CONFIG.whichKey.sortBy,
      },
      bindings,
    },
    diagnostics,
  };
}

/** The config as a layer, ready to append after the built-in ones. */
export function configLayer(config: KeybindingsConfig): Layer {
  return { source: "config", bindings: config.bindings };
}

/** Per-device overrides written by the keybindings view. */
export function userLayer(bindings: readonly BindingSpec[]): Layer {
  return { source: "user", bindings };
}
