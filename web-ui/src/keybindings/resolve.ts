import {
  applyPrimaryModifier,
  ChordParseError,
  expandLeaders,
  formatChord,
  parseChord,
} from "./chord";
import type {
  BindingSpec,
  Conflict,
  Host,
  Layer,
  Mode,
  ResolvedBinding,
} from "./types";

/**
 * Layer composition.
 *
 * Layers apply in order — base, platform, target, host quirks, config file,
 * user. Within the built-in layers a later binding for the same command and
 * `when` scope supersedes the earlier one, which is what lets the web layer
 * bend a canonical chord around a browser without restating the whole map. The
 * config and user layers add rather than supersede, so a user keeps the default
 * chord unless they remove it with a `-command` entry.
 */

const BUILT_IN = new Set(["base", "platform", "target", "host"]);

export interface ResolveOptions {
  readonly host: Host;
  readonly mode: Mode;
  /** Authored leader chords. Default: Space and comma. */
  readonly leader?: string;
  readonly localLeader?: string;
}

export interface ResolveResult {
  readonly bindings: readonly ResolvedBinding[];
  /** Entries that could not be applied. Surfaced in the keybindings view. */
  readonly rejected: readonly Conflict[];
}

function applies(spec: BindingSpec, host: Host, mode: Mode): boolean {
  if (spec.mode && spec.mode !== mode) return false;
  if (spec.platform && spec.platform !== host.platform) return false;
  if (spec.target && spec.target !== host.target) return false;
  if (spec.browser && spec.browser !== host.browser) return false;
  return true;
}

interface Staged extends ResolvedBinding {
  /** Index of the layer that contributed this entry. */
  readonly layer: number;
}

function sameScope(a: ResolvedBinding, command: string, when?: string, mode?: Mode): boolean {
  return a.command === command && a.when === when && a.mode === mode;
}

/** Apply a `-command` entry. `-*` clears everything resolved so far. */
function remove(out: Staged[], spec: BindingSpec, chordId: string | undefined): void {
  const target = spec.command.slice(1);
  if (target === "*") {
    out.length = 0;
    return;
  }
  for (let i = out.length - 1; i >= 0; i -= 1) {
    const entry = out[i];
    if (entry.command !== target) continue;
    if (chordId && entry.id !== chordId) continue;
    out.splice(i, 1);
  }
}

export function resolve(layers: readonly Layer[], options: ResolveOptions): ResolveResult {
  const { host, mode } = options;
  const leader = parseChord(options.leader ?? "<space>");
  const localLeader = parseChord(options.localLeader ?? ",");

  const out: Staged[] = [];
  const rejected: Conflict[] = [];

  for (const [index, layer] of layers.entries()) {
    for (const spec of layer.bindings) {
      if (!applies(spec, host, mode)) continue;

      const isRemoval = spec.command.startsWith("-");
      let seq;
      try {
        const authored = applyPrimaryModifier(spec.key, host.platform);
        seq = expandLeaders(parseChord(authored), leader, localLeader);
      } catch (error) {
        if (!(error instanceof ChordParseError)) throw error;
        rejected.push({
          kind: "malformed",
          chord: spec.key,
          detail: error.message,
          commands: [spec.command],
        });
        continue;
      }

      const id = formatChord(seq);
      if (isRemoval) {
        remove(out, spec, spec.key === "*" ? undefined : id);
        continue;
      }

      // Superseding is across layers only. Two chords authored side by side in
      // one layer are both intentional — F1 alongside mod+shift+p, `i` and `a`
      // both entering the composer — and must survive.
      if (BUILT_IN.has(layer.source)) {
        for (let i = out.length - 1; i >= 0; i -= 1) {
          const entry = out[i];
          if (entry.layer === index) continue;
          if (sameScope(entry, spec.command, spec.when, spec.mode)) out.splice(i, 1);
        }
      }

      out.push({
        seq,
        id,
        command: spec.command,
        when: spec.when,
        mode: spec.mode,
        group: spec.group,
        label: spec.label,
        source: layer.source,
        layer: index,
      });
    }
  }

  return { bindings: out, rejected };
}
