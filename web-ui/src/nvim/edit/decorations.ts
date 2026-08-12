import { Decoration, EditorView, WidgetType, type DecorationSet } from "@codemirror/view";
import { StateEffect, StateField, type EditorState, type Range } from "@codemirror/state";
import { nextGraphemeOffset, positionToOffset } from "./columns";
import { emptyOverlays, searchPattern, type OverlayState } from "./overlays";
import type { ModeShort, TextPosition, VisualSelection } from "./wire";
import { NVIM_CAPTURE_ATTRIBUTE } from "../capture";

export type IdleReason =
  | "disabled"
  | "engine-codemirror"
  | "mobile-surface"
  | "no-file"
  | "no-session"
  | "not-code-surface";

export type ConnectionState =
  | { readonly status: "idle"; readonly reason: IdleReason }
  | { readonly status: "connecting" }
  | { readonly status: "attached" }
  | { readonly status: "failed"; readonly reason: string };

export interface BindingSnapshot {
  readonly mode: string;
  readonly modeShort: ModeShort;
  readonly cursor: TextPosition | null;
  readonly visual: VisualSelection | null;
  readonly changedtick: number;
  readonly connection: ConnectionState;
  readonly message: string | null;
  readonly overlays: OverlayState;
}

export const initialBindingSnapshot: BindingSnapshot = {
  mode: "normal",
  modeShort: "normal",
  cursor: null,
  visual: null,
  changedtick: 0,
  connection: { status: "idle", reason: "disabled" },
  message: null,
  overlays: emptyOverlays,
};

export const bindingSnapshotEffect = StateEffect.define<BindingSnapshot>();

export const bindingStateField = StateField.define<BindingSnapshot>({
  create: () => initialBindingSnapshot,
  update(value, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(bindingSnapshotEffect)) return effect.value;
    }
    return value;
  },
  provide: (field) => EditorView.editorAttributes.from(field, captureAttributes),
});

function captureAttributes(snapshot: BindingSnapshot): Record<string, string> {
  return snapshot.connection.status === "attached" ? { [NVIM_CAPTURE_ATTRIBUTE]: "true" } : {};
}

class CursorMarker extends WidgetType {
  constructor(private readonly className: string) {
    super();
  }

  toDOM(): HTMLElement {
    const element = document.createElement("span");
    element.className = this.className;
    element.setAttribute("aria-hidden", "true");
    return element;
  }

  eq(other: unknown): boolean {
    return other instanceof CursorMarker && other.className === this.className;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

function cursorRanges(state: EditorState, snapshot: BindingSnapshot): Range<Decoration>[] {
  if (!snapshot.cursor) return [];
  const offset = positionToOffset(state.doc, snapshot.cursor);
  const line = state.doc.lineAt(offset);
  const next = nextGraphemeOffset(line.text, offset - line.from) + line.from;
  const mode = snapshot.modeShort;
  const className = mode === "insert"
    ? "cm-nvim-cursor cm-nvim-cursor-bar"
    : mode === "replace"
      ? "cm-nvim-cursor cm-nvim-cursor-underline"
      : "cm-nvim-cursor cm-nvim-cursor-block";
  if (next > offset) return [Decoration.mark({ class: className }).range(offset, next)];
  return [Decoration.widget({ widget: new CursorMarker(className), side: 1 }).range(offset)];
}

function visualRanges(state: EditorState, visual: VisualSelection, mode: ModeShort): Range<Decoration>[] {
  const className = "cm-nvim-visual-selection";
  const start = positionToOffset(state.doc, visual.start);
  const end = positionToOffset(state.doc, visual.end);
  if (mode === "visual_line") {
    const first = Math.min(visual.start.line, visual.end.line);
    const last = Math.max(visual.start.line, visual.end.line);
    const ranges: Range<Decoration>[] = [];
    for (let lineNumber = first; lineNumber <= last && lineNumber < state.doc.lines; lineNumber += 1) {
      const line = state.doc.line(lineNumber + 1);
      if (line.from < line.to) ranges.push(Decoration.mark({ class: className }).range(line.from, line.to));
    }
    return ranges;
  }
  if (mode === "visual_block") {
    const firstLine = Math.min(visual.start.line, visual.end.line);
    const lastLine = Math.max(visual.start.line, visual.end.line);
    const firstColumn = Math.min(visual.start.column, visual.end.column);
    const lastColumn = Math.max(visual.start.column, visual.end.column);
    const ranges: Range<Decoration>[] = [];
    for (let lineNumber = firstLine; lineNumber <= lastLine && lineNumber < state.doc.lines; lineNumber += 1) {
      const line = state.doc.line(lineNumber + 1);
      const from = Math.min(line.to, line.from + firstColumn);
      const to = Math.min(line.to, line.from + lastColumn + 1);
      if (from < to) ranges.push(Decoration.mark({ class: className }).range(from, to));
    }
    return ranges;
  }
  const from = Math.min(start, end);
  const to = Math.max(start, end);
  if (from === to) {
    const line = state.doc.lineAt(from);
    if (from === line.to) return [];
    const endExclusive = nextGraphemeOffset(line.text, from - line.from) + line.from;
    return [Decoration.mark({ class: className }).range(from, endExclusive)];
  }
  const endLine = state.doc.lineAt(to);
  const endExclusive = to < endLine.to ? nextGraphemeOffset(endLine.text, to - endLine.from) + endLine.from : to;
  return [Decoration.mark({ class: className }).range(from, endExclusive)];
}

const MAX_SEARCH_MATCHES = 2_000;

/** A Vim pattern as a JavaScript matcher, falling back to a literal scan. */
function searchMatcher(pattern: string): RegExp | null {
  if (pattern.length === 0) return null;
  const literal = new RegExp(pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "g");
  try {
    return new RegExp(pattern, "g");
  } catch {
    return literal;
  }
}

function searchRanges(state: EditorState, pattern: string | null): Range<Decoration>[] {
  const matcher = pattern === null ? null : searchMatcher(pattern);
  if (!matcher) return [];
  const decoration = Decoration.mark({ class: "cm-searchMatch" });
  const ranges: Range<Decoration>[] = [];
  const text = state.doc.toString();
  for (const match of text.matchAll(matcher)) {
    if (match.index === undefined || match[0].length === 0) continue;
    ranges.push(decoration.range(match.index, match.index + match[0].length));
    if (ranges.length === MAX_SEARCH_MATCHES) break;
  }
  return ranges;
}

export function decorationsFor(state: EditorState, snapshot: BindingSnapshot): DecorationSet {
  const ranges = searchRanges(state, searchPattern(snapshot.overlays));
  ranges.push(...cursorRanges(state, snapshot));
  if (snapshot.visual) ranges.push(...visualRanges(state, snapshot.visual, snapshot.modeShort));
  return Decoration.set(ranges, true);
}

export const bindingDecorations = StateField.define<DecorationSet>({
  create: (state) => decorationsFor(state, initialBindingSnapshot),
  update(value, transaction) {
    const nextSnapshot = transaction.effects.find((effect) => effect.is(bindingSnapshotEffect));
    if (nextSnapshot) return decorationsFor(transaction.state, nextSnapshot.value);
    return transaction.docChanged ? value.map(transaction.changes) : value;
  },
  provide: (field) => EditorView.decorations.from(field),
});
