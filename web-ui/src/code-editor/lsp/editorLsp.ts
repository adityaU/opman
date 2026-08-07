/**
 * editorLsp — LSP as editor behaviour rather than toolbar buttons.
 *
 * The header used to carry an "issues" pill and three icon buttons: hover,
 * go-to-definition, and format. Each of them asked the reader to move the
 * cursor, look away to the top-right corner, press a glyph, then look back —
 * for information that belongs at the token they were already looking at.
 *
 * These extensions put every one of them in the file area: hover on the word
 * under the pointer, definition on ⌘-click or F12, format on ⇧⌥F, and
 * diagnostics underlined in place with a gutter marker.
 */
import { hoverTooltip, keymap, EditorView, type Tooltip } from "@codemirror/view";
import { lintGutter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import type { EditorLspDiagnostic } from "../types";
import { markdownElement } from "./lspMarkdown";
import { lspCompletionExtension, type LspCompletionResponse } from "./lspCompletion";

/**
 * Live handles into the panel. Extensions are built once and read through
 * this ref, so they never go stale as the open file or session changes.
 */
export interface LspBridge {
  enabled: boolean;
  hoverAt: (line: number, col: number) => Promise<string | null>;
  definitionAt: (line: number, col: number) => Promise<boolean>;
  format: () => Promise<void>;
  completeAt: (
    line: number,
    col: number,
    trigger?: string,
  ) => Promise<LspCompletionResponse | null>;
  /** Characters the server wants completions re-queried on, e.g. `.` and `:`. */
  triggerCharacters: () => string[];
}

export type LspBridgeRef = { current: LspBridge };

const SEVERITY: Record<string, Diagnostic["severity"]> = {
  error: "error",
  warning: "warning",
  info: "info",
  hint: "hint",
};

/** Translate a 1-based line/col pair into a document offset, clamped. */
function offsetAt(doc: any, lnum: number, col: number): number {
  const line = doc.line(Math.max(1, Math.min(lnum, doc.lines)));
  return Math.min(line.to, line.from + Math.max(0, col - 1));
}

/** Turn a document offset back into the 1-based line/col the server expects. */
function positionAt(doc: any, pos: number): { line: number; col: number } {
  const line = doc.lineAt(pos);
  return { line: line.number, col: pos - line.from + 1 };
}

// ── Diagnostics ─────────────────────────────────────────

/**
 * Underline the server's diagnostics in the text and mark them in the gutter.
 * Call whenever the diagnostic set changes; it is a no-op on an unmounted view.
 */
export function pushDiagnostics(view: any, list: EditorLspDiagnostic[]) {
  if (!view?.state?.doc) return;
  const doc = view.state.doc;
  const mapped: Diagnostic[] = list.map((d) => {
    const from = offsetAt(doc, d.lnum, d.col);
    const line = doc.lineAt(from);
    return {
      from,
      to: Math.max(from + 1, Math.min(line.to, from + 1)),
      severity: SEVERITY[d.severity?.toLowerCase()] ?? "error",
      message: d.source ? `${d.message}\n\n${d.source}` : d.message,
    };
  });
  view.dispatch(setDiagnostics(view.state, mapped));
}

// ── Extensions ──────────────────────────────────────────

export function createLspExtensions(bridge: LspBridgeRef) {
  const tooltip = hoverTooltip(async (view, pos): Promise<Tooltip | null> => {
    if (!bridge.current.enabled) return null;
    const { line, col } = positionAt(view.state.doc, pos);
    const text = await bridge.current.hoverAt(line, col);
    if (!text) return null;
    return {
      pos,
      above: true,
      // Servers answer in markdown — a fenced signature block, then prose.
      // Printed literally that is a wall of backticks.
      create: () => ({ dom: markdownElement(text, "cm-lsp-hover") }),
    };
  }, { hoverTime: 320 });

  const goToDefinition = (view: EditorView, pos: number) => {
    if (!bridge.current.enabled) return false;
    const { line, col } = positionAt(view.state.doc, pos);
    void bridge.current.definitionAt(line, col);
    return true;
  };

  const keys = keymap.of([
    {
      key: "F12",
      run: (view) => goToDefinition(view, view.state.selection.main.head),
    },
    {
      key: "Shift-Alt-f",
      run: () => {
        if (!bridge.current.enabled) return false;
        void bridge.current.format();
        return true;
      },
    },
  ]);

  // ⌘/Ctrl-click jumps to the definition of whatever is under the pointer.
  const clickToDefine = EditorView.domEventHandlers({
    mousedown(event, view) {
      if (!(event.metaKey || event.ctrlKey) || event.button !== 0) return false;
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos == null) return false;
      event.preventDefault();
      return goToDefinition(view, pos);
    },
  });

  // The pointer only claims a link affordance while the modifier is actually
  // held, so ordinary reading never looks clickable.
  const modifierHint = EditorView.contentAttributes.of({ "data-lsp": "on" });

  return [
    tooltip,
    lintGutter(),
    keys,
    clickToDefine,
    modifierHint,
    lspCompletionExtension(bridge),
  ];
}
