import { isPrefixOf, stepsEqual } from "./chord";
import type { ChordStep, CommandId, Mode, ResolvedBinding } from "./types";

/**
 * The chord state machine.
 *
 * It owns one question — given the steps pressed so far, does this key run
 * something, extend a pending chord, or fall through? — and answers it without
 * touching the DOM, which is what makes it testable and what lets the which-key
 * hints read from the same source as dispatch.
 */

/** Context keys the surfaces publish, evaluated by the host. */
export interface MatchContext {
  readonly mode: Mode;
  /** True while a text input, textarea or contenteditable holds focus. */
  readonly textInput: boolean;
  /** Evaluates a `when` clause. Absent clauses are always true. */
  readonly isTrue: (clause: string) => boolean;
}

export type MatchResult =
  | { readonly type: "run"; readonly command: CommandId; readonly binding: ResolvedBinding }
  | { readonly type: "pending"; readonly steps: readonly ChordStep[] }
  | { readonly type: "none" };

/** Context keys that name a text-input surface and so survive the insert guard. */
const INSERT_SAFE_CLAUSES = ["composerFocused", "findOpen", "terminalFindOpen"];

function hasModifier(step: ChordStep): boolean {
  return step.ctrl || step.meta || step.alt;
}

/**
 * While a text input holds focus, a bare key must type rather than dispatch.
 * A binding escapes that guard by carrying a modifier, by being Escape, or by
 * naming the input surface it belongs to — Enter to send is a binding on the
 * composer, not a global.
 */
function survivesInsertGuard(binding: ResolvedBinding): boolean {
  const first = binding.seq[0];
  if (hasModifier(first)) return true;
  if (first.key === "escape") return true;
  if (!binding.when) return false;
  return INSERT_SAFE_CLAUSES.some((clause) => binding.when?.includes(clause));
}

function isLive(binding: ResolvedBinding, ctx: MatchContext): boolean {
  if (binding.mode && binding.mode !== ctx.mode) return false;
  if (ctx.textInput && !survivesInsertGuard(binding)) return false;
  if (!binding.when) return true;
  return ctx.isTrue(binding.when);
}

/**
 * A scoped binding beats an unscoped one on the same chord. This is the
 * dispatch-time half of the rule the conflict validator relies on: two
 * differently-scoped bindings may share a chord precisely because the more
 * specific one wins here.
 */
function mostSpecific(candidates: readonly ResolvedBinding[]): ResolvedBinding | undefined {
  if (candidates.length <= 1) return candidates[0];
  return candidates.find((b) => b.when) ?? candidates[0];
}

export class Keymap {
  private readonly bindings: readonly ResolvedBinding[];

  constructor(bindings: readonly ResolvedBinding[]) {
    this.bindings = bindings;
  }

  /** Bindings whose chord is exactly `steps`. */
  private exact(steps: readonly ChordStep[]): ResolvedBinding[] {
    return this.bindings.filter(
      (b) => b.seq.length === steps.length && b.seq.every((s, i) => stepsEqual(s, steps[i])),
    );
  }

  /** Bindings whose chord continues past `steps`. */
  private extensions(steps: readonly ChordStep[]): ResolvedBinding[] {
    if (steps.length === 0) return [...this.bindings];
    return this.bindings.filter((b) => isPrefixOf(steps, b.seq));
  }

  /**
   * Feed one key press. `pending` is the sequence accumulated so far, which the
   * caller holds; the matcher stays pure so a hint panel can ask "what would
   * happen next" without advancing anything.
   */
  match(pending: readonly ChordStep[], step: ChordStep, ctx: MatchContext): MatchResult {
    const steps = [...pending, step];

    // A live continuation wins over an exact match, so a prefix can never fire
    // early. The conflict validator guarantees the two are never both live.
    const waiting = this.extensions(steps).filter((b) => isLive(b, ctx));
    if (waiting.length > 0) return { type: "pending", steps };

    const binding = mostSpecific(this.exact(steps).filter((b) => isLive(b, ctx)));
    if (binding) return { type: "run", command: binding.command, binding };

    return { type: "none" };
  }

  /** Every live binding reachable from `pending`, for the which-key panel. */
  continuations(pending: readonly ChordStep[], ctx: MatchContext): ResolvedBinding[] {
    return this.extensions(pending).filter((b) => isLive(b, ctx));
  }

  /** Chords bound to a command, for the palette and the cheatsheet. */
  chordsFor(command: CommandId): ResolvedBinding[] {
    return this.bindings.filter((b) => b.command === command);
  }

  get all(): readonly ResolvedBinding[] {
    return this.bindings;
  }
}

export interface HintGroup {
  readonly group: string;
  readonly entries: readonly HintEntry[];
}

export interface HintEntry {
  /** The next key to press, as a display string. */
  readonly key: ChordStep;
  readonly label: string;
  /** True when this key opens another level rather than running a command. */
  readonly isPrefix: boolean;
}

/**
 * Build the which-key hint set for a pending chord.
 *
 * Entries are keyed by the single next step, so a namespace collapses into one
 * row rather than one row per leaf beneath it.
 */
export function buildHints(
  keymap: Keymap,
  pending: readonly ChordStep[],
  ctx: MatchContext,
  labelFor: (binding: ResolvedBinding) => string,
): HintGroup[] {
  const byKey = new Map<string, { step: ChordStep; bindings: ResolvedBinding[] }>();

  for (const binding of keymap.continuations(pending, ctx)) {
    const next = binding.seq[pending.length];
    if (!next) continue;
    const id = `${next.ctrl}${next.shift}${next.alt}${next.meta}${next.key}`;
    const entry = byKey.get(id);
    if (entry) entry.bindings.push(binding);
    else byKey.set(id, { step: next, bindings: [binding] });
  }

  const groups = new Map<string, HintEntry[]>();
  for (const { step, bindings } of byKey.values()) {
    const isPrefix = bindings.some((b) => b.seq.length > pending.length + 1);
    const leaf = bindings.find((b) => b.seq.length === pending.length + 1);
    const group = (leaf ?? bindings[0]).group ?? "other";
    const label = isPrefix && !leaf ? `+${group}` : labelFor(leaf ?? bindings[0]);

    const existing = groups.get(group);
    const entry: HintEntry = { key: step, label, isPrefix };
    if (existing) existing.push(entry);
    else groups.set(group, [entry]);
  }

  return [...groups.entries()]
    .map(([group, entries]) => ({ group, entries }))
    .sort((a, b) => a.group.localeCompare(b.group));
}
