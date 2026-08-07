import { formatStep, isPrefixOf } from "./chord";
import { reservedOwner } from "./host";
import type { CommandDef, Conflict, Host, ResolvedBinding } from "./types";

/**
 * Validation of a composed keymap.
 *
 * Two bindings collide only when they are scoped identically. A more specific
 * binding shadows a less specific one instead of fighting it, which is what
 * makes Ctrl+B bold inside the document editor while still toggling the sidebar
 * everywhere else, and what lets Escape mean "stop generating" over "close
 * panel" without either being wrong.
 *
 * The cost is that two different clauses are assumed disjoint. Deciding
 * otherwise needs a real expression evaluator, and the matcher already resolves
 * the ambiguity at dispatch time by preferring the most specific live binding.
 */
function scopesOverlap(a: ResolvedBinding, b: ResolvedBinding): boolean {
  if (a.mode && b.mode && a.mode !== b.mode) return false;
  return a.when === b.when;
}

function duplicates(bindings: readonly ResolvedBinding[]): Conflict[] {
  const byChord = new Map<string, ResolvedBinding[]>();
  for (const binding of bindings) {
    const group = byChord.get(binding.id);
    if (group) group.push(binding);
    else byChord.set(binding.id, [binding]);
  }

  const out: Conflict[] = [];
  for (const [chord, group] of byChord) {
    if (group.length < 2) continue;
    for (let i = 0; i < group.length; i += 1) {
      for (let j = i + 1; j < group.length; j += 1) {
        const [a, b] = [group[i], group[j]];
        if (a.command === b.command) continue;
        if (!scopesOverlap(a, b)) continue;
        out.push({
          kind: "duplicate",
          chord,
          detail: `bound to both ${a.command} and ${b.command} in the same scope`,
          commands: [a.command, b.command],
        });
      }
    }
  }
  return out;
}

/**
 * Invariant (a): no node is both a prefix and a command. A chord that runs
 * something can never be waited on, so `Space e` cannot focus the explorer
 * while `Space e f` creates a file.
 */
function prefixIsCommand(bindings: readonly ResolvedBinding[]): Conflict[] {
  const out: Conflict[] = [];
  for (const shorter of bindings) {
    for (const longer of bindings) {
      if (shorter === longer) continue;
      if (!isPrefixOf(shorter.seq, longer.seq)) continue;
      if (!scopesOverlap(shorter, longer)) continue;
      out.push({
        kind: "prefix-is-command",
        chord: shorter.id,
        detail: `runs ${shorter.command} but is also the prefix of ${longer.id} (${longer.command})`,
        commands: [shorter.command, longer.command],
      });
    }
  }
  return out;
}

/**
 * A reserved chord is stolen at any position in a sequence, not just the first:
 * a pending `ctrl+k` does not stop the browser acting on `ctrl+w`.
 */
function reserved(bindings: readonly ResolvedBinding[], host: Host): Conflict[] {
  const out: Conflict[] = [];
  for (const binding of bindings) {
    for (const step of binding.seq) {
      const id = formatStep(step);
      const owner = reservedOwner(host, id);
      if (!owner) continue;
      out.push({
        kind: "reserved",
        chord: binding.id,
        detail: `step "${id}" is taken by ${owner} on ${host.platform}/${host.browser}`,
        commands: [binding.command],
      });
    }
  }
  return out;
}

function unknownCommands(
  bindings: readonly ResolvedBinding[],
  commands: readonly CommandDef[],
): Conflict[] {
  const known = new Set(commands.map((c) => c.id));
  return bindings
    .filter((b) => !known.has(b.command))
    .map((b) => ({
      kind: "unknown-command" as const,
      chord: b.id,
      detail: `no command registered with id "${b.command}"`,
      commands: [b.command],
    }));
}

export interface ValidateInput {
  readonly bindings: readonly ResolvedBinding[];
  readonly host: Host;
  readonly commands: readonly CommandDef[];
}

export function validate({ bindings, host, commands }: ValidateInput): Conflict[] {
  return [
    ...duplicates(bindings),
    ...prefixIsCommand(bindings),
    ...reserved(bindings, host),
    ...unknownCommands(bindings, commands),
  ];
}
