/**
 * A pane's trail: everywhere it has been pointed, in order.
 *
 * A pane used to remember exactly one thing about its past — the target it is
 * showing right now. Opening a second file forgot the first; pointing a pane at
 * a chat session forgot the shell it had been attached to, even though the
 * shell was still running on the server. The trail existed in the user's head
 * and nowhere in the model.
 *
 * It is per pane rather than per widget kind, so it crosses kinds: back from a
 * chat session lands on the shell that pane was showing before, not on the
 * previous session. And it is one flat list with a cursor, exactly like a
 * browser's, because that is the behaviour every user already has an intuition
 * for — going back and then somewhere new discards the forward tail.
 *
 * Everything here is pure and total, so the whole of "what does back do" is
 * testable without rendering a pane.
 */

import { basename } from "../utils/path";
import type { WidgetState } from "./types";

/**
 * Older entries fall off the front. The cap is generous enough that no real
 * session reaches it and small enough that the whole workspace still serialises
 * to a few kilobytes of localStorage with a dozen panes in it.
 */
const LIMIT = 24;

/** How many past targets the pane menu offers as direct jumps. */
const RECENT_LIMIT = 8;

/**
 * Invariant, asserted in the tests and repaired on load: `entries[index]` is
 * the pane's own widget, or `index === entries.length` and the pane is empty.
 *
 * That is what makes going back total rather than best-effort. Without it a
 * pane could sit on a widget its own trail disagrees with, and every step from
 * there would compound the disagreement.
 *
 * One past the end is how "showing nothing" is spelled, rather than -1 or a
 * separate flag. It costs no extra state and it makes the useful case fall out:
 * back from a pane you have just cleared reaches the newest entry, which is
 * what an accidental close wants, while forward correctly has nowhere to go.
 */
export interface PaneHistory {
  /** Oldest → newest. */
  readonly entries: readonly WidgetState[];
  /** Cursor into `entries`. `entries.length` means the pane is showing nothing. */
  readonly index: number;
}

export const EMPTY_HISTORY: PaneHistory = { entries: [], index: 0 };

// ── Identity ────────────────────────────────────────────

/**
 * Whether two widgets are the same *place*.
 *
 * Deliberately narrower than deep equality: it compares what the user would
 * call the destination and ignores everything else the arm carries. A chat
 * pane's engine and a files pane's LSP scope are settings of a place, not
 * places of their own, so changing one must not push an entry — and the panels
 * that report on every settle (`onPtyIdChanged`, `onBrowserUrlChanged`) must
 * not flood the list with the target they are already on.
 */
export function sameTarget(a: WidgetState | null, b: WidgetState | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  if (a.kind !== b.kind || a.projectPath !== b.projectPath) return false;

  switch (a.kind) {
    case "chat":
      return b.kind === "chat" && a.sessionId === b.sessionId;
    case "files":
      return b.kind === "files" && (a.open?.path ?? null) === (b.open?.path ?? null);
    case "terminal":
      return b.kind === "terminal" && a.ptyId === b.ptyId;
    case "browser":
      return b.kind === "browser" && a.browserId === b.browserId && a.url === b.url;
    case "git":
      return true;
  }
}

// ── Recording ───────────────────────────────────────────

/**
 * Record that a pane is now showing `widget`.
 *
 * A repeat of the current target replaces it in place rather than pushing, so
 * the detail a panel just learned — a line number, a resolved URL — lands on
 * the entry the pane is already on. Anything else truncates the forward tail
 * and appends, which is the one rule a browser's back button has taught
 * everybody.
 *
 * A null widget is never an entry: a blank pane is not a place. Clearing a pane
 * therefore keeps its trail, and back reopens what was there — which is the
 * behaviour an accidental close wants.
 */
export function recordTarget(history: PaneHistory, widget: WidgetState | null): PaneHistory {
  if (!widget) return history;

  const current = currentTarget(history);
  if (sameTarget(current, widget)) {
    if (current === widget) return history;
    const entries = [...history.entries];
    entries[history.index] = widget;
    return { entries, index: history.index };
  }

  const kept = history.entries.slice(0, history.index + 1);
  kept.push(widget);
  const entries = kept.length > LIMIT ? kept.slice(kept.length - LIMIT) : kept;
  return { entries, index: entries.length - 1 };
}

/**
 * Update the current entry without moving the cursor.
 *
 * The counterpart to `recordTarget` for a write that is not navigation at all —
 * a chat pane's engine, or the session id filled in once the first send has
 * created it. Both describe the place the pane is already on, and pushing for
 * them would put the same conversation in the list twice.
 */
export function amendTarget(history: PaneHistory, widget: WidgetState | null): PaneHistory {
  if (!widget || history.index >= history.entries.length) return history;
  const entries = [...history.entries];
  entries[history.index] = widget;
  return { entries, index: history.index };
}

/**
 * The pane is now showing nothing, but has been somewhere.
 *
 * The cursor parks one past the end rather than the trail being dropped, so the
 * pane an opener emptied — or one whose widget was cleared by mistake — is one
 * Back away from what it held.
 */
export function clearTarget(history: PaneHistory): PaneHistory {
  const index = history.entries.length;
  return history.index === index ? history : { ...history, index };
}

export function currentTarget(history: PaneHistory): WidgetState | null {
  return history.entries[history.index] ?? null;
}

// ── Navigating ──────────────────────────────────────────

export function canStep(history: PaneHistory, step: 1 | -1): boolean {
  const next = history.index + step;
  return next >= 0 && next < history.entries.length;
}

/** Where a step would land, for labelling the Back and Forward rows. */
export function peekStep(history: PaneHistory, step: 1 | -1): WidgetState | null {
  return history.entries[history.index + step] ?? null;
}

/**
 * Move the cursor and hand back the widget the pane should now show.
 *
 * `seq` re-mints the token the panels use to notice a reveal they have already
 * seen once (see `refreshTarget`). Returns `null` when there is nowhere to go,
 * so the caller has one thing to check rather than two.
 */
export function stepHistory(
  history: PaneHistory,
  step: 1 | -1,
  seq: number,
): { readonly history: PaneHistory; readonly widget: WidgetState } | null {
  return jumpHistory(history, history.index + step, seq);
}

export function jumpHistory(
  history: PaneHistory,
  index: number,
  seq: number,
): { readonly history: PaneHistory; readonly widget: WidgetState } | null {
  const entry = history.entries[index];
  if (!entry) return null;
  const widget = refreshTarget(entry, seq);
  // The entry is rewritten with its fresh token so the invariant still holds:
  // the pane's widget and `entries[index]` stay the same object.
  const entries = [...history.entries];
  entries[index] = widget;
  return { history: { entries, index }, widget };
}

/**
 * Re-arm an entry so the panel showing it acts on it again.
 *
 * Two panels ignore a target that goes backwards, both on purpose. The editor's
 * reveal effect is keyed on `open.seq`, so an older request looks like one it
 * has already handled. The browser's tab keeps its own page and treats the
 * widget's URL as a starting point, so writing an earlier URL back changes
 * nothing. Both watch a counter instead, and a jump bumps it.
 */
export function refreshTarget(widget: WidgetState, seq: number): WidgetState {
  if (widget.kind === "files") {
    return widget.open ? { ...widget, open: { ...widget.open, seq } } : widget;
  }
  if (widget.kind === "browser") {
    return widget.url ? { ...widget, reveal: seq } : widget;
  }
  return widget;
}

/**
 * Past targets as direct jumps, newest first, with the one the pane is on left
 * out — it is not somewhere to go.
 */
export function recentTargets(
  history: PaneHistory,
): readonly { readonly index: number; readonly widget: WidgetState }[] {
  const out: { index: number; widget: WidgetState }[] = [];
  for (let index = history.entries.length - 1; index >= 0; index -= 1) {
    if (index === history.index) continue;
    out.push({ index, widget: history.entries[index] });
    if (out.length === RECENT_LIMIT) break;
  }
  return out;
}

// ── Labelling ───────────────────────────────────────────

/**
 * What to call a target, from the widget alone.
 *
 * Never stored on the entry: a session's title and a shell's name live on the
 * server and change without the workspace hearing about it, so a copy here
 * would be a stale copy. The pane menu passes a resolver that improves the two
 * kinds needing a lookup; this is the answer when there is none, and the whole
 * answer for the three kinds that need none.
 */
export function targetLabel(widget: WidgetState): string {
  switch (widget.kind) {
    case "files":
      return widget.open ? basename(widget.open.path) : "Files";
    case "terminal":
      return widget.ptyId ? "Shell" : "New shell";
    case "browser":
      return widget.url ? hostOf(widget.url) : "Browser";
    case "chat":
      return widget.sessionId ? `Session ${widget.sessionId.slice(0, 8)}` : "New session";
    case "git":
      return "Git";
  }
}

function hostOf(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

// ── Loading ─────────────────────────────────────────────

/**
 * Reconcile a restored trail with the widget it is supposed to belong to.
 *
 * The two are persisted side by side and could disagree — an older layout with
 * no trail at all, a hand-edited value, a write that landed half-done. Rather
 * than trust either, the widget wins and the trail is made consistent with it,
 * because the widget is what the pane will actually render.
 */
export function repairHistory(history: PaneHistory, widget: WidgetState | null): PaneHistory {
  if (!widget) return clearTarget(history);
  if (sameTarget(currentTarget(history), widget)) return amendTarget(history, widget);
  return recordTarget(history, widget);
}
