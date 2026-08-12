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
import { forEachDiagnostic, lintGutter, linter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import type { EditorLspDiagnostic } from "../types";
import type { EditorReferenceLocation } from "../../api";
import { markdownElement } from "./lspMarkdown";
import { diagnosticHoverExtension } from "./diagnosticHover";
import { lspCompletionExtension, type LspCompletionResponse } from "./lspCompletion";
import {
  closeLspPanel, lspPanelExtension, openLspPanel, wordAtCursor,
  type LspPanelBridgeRef,
} from "./lspPanel";

/**
 * Live handles into the panel. Extensions are built once and read through
 * this ref, so they never go stale as the open file or session changes.
 */
export interface LspBridge {
  enabled: boolean;
  diagnostics: () => readonly EditorLspDiagnostic[];
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
  /** Every use of the symbol at a position, for the references panel. */
  referencesAt: (line: number, col: number) => Promise<readonly EditorReferenceLocation[]>;
  /** Rename the symbol at a position across the project. Resolves true on a write. */
  renameAt: (line: number, col: number, name: string) => Promise<boolean>;
  /** Open a file at a line, for a reference row. */
  jumpTo: (file: string, line: number) => void;
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

/** Actions a Neovim mapping can name, filled in when the extensions mount. */
export interface LspActions {
  run: (name: string) => void;
}

export function createLspExtensions(bridge: LspBridgeRef, actions: LspActions) {
  const tooltip = hoverTooltip(async (view, pos): Promise<Tooltip | null> => {
    const diagnostic = diagnosticAt(view, pos);
    if (diagnostic) return diagnosticTooltip(pos, diagnostic);
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
  }, { hoverTime: 0 });

  const goToDefinition = (view: EditorView, pos: number) => {
    const { line, col } = positionAt(view.state.doc, pos);
    void bridge.current.definitionAt(line, col);
    return true;
  };

  const panelBridge: LspPanelBridgeRef = {
    current: {
      jumpTo: (file, line) => bridge.current.jumpTo(file, line),
      applyRename: () => {},
    },
  };

  const showReferences = (view: EditorView): boolean => {
    const { line, col } = positionAt(view.state.doc, view.state.selection.main.head);
    openLspPanel(view, { kind: "references", locations: [], busy: true });
    void bridge.current.referencesAt(line, col).then((locations) => {
      openLspPanel(view, { kind: "references", locations, busy: false });
    });
    return true;
  };

  const startRename = (view: EditorView): boolean => {
    const { line, col } = positionAt(view.state.doc, view.state.selection.main.head);
    panelBridge.current = {
      jumpTo: (file, target) => bridge.current.jumpTo(file, target),
      applyRename: (name) => {
        openLspPanel(view, { kind: "rename", name, busy: true });
        void bridge.current.renameAt(line, col, name).then(() => closeLspPanel(view));
      },
    };
    openLspPanel(view, { kind: "rename", name: wordAtCursor(view), busy: false });
    return true;
  };

  // In insert mode keystrokes stay local, so Neovim's mappings cannot fire and
  // CodeMirror needs the same three shortcuts. In every other mode Neovim
  // consumes the key first and calls back through `actions.run`.
  const keys = keymap.of([
    { key: "F12", run: (view) => goToDefinition(view, view.state.selection.main.head) },
    { key: "Shift-F12", run: showReferences },
    { key: "F2", run: startRename },
    { key: "Escape", run: closeLspPanel },
    {
      key: "Shift-Alt-f",
      run: () => {
        if (!bridge.current.enabled) return false;
        void bridge.current.format();
        return true;
      },
    },
  ]);

  // The action handler needs whichever view is mounted, and only the view
  // itself can say which that is.
  const latestView: { current: EditorView | null } = { current: null };
  const trackView = EditorView.updateListener.of((update) => { latestView.current = update.view; });

  actions.run = (name: string): void => {
    const view = latestView.current;
    if (!view) return;
    if (name === "lsp.definition") goToDefinition(view, view.state.selection.main.head);
    else if (name === "lsp.references") showReferences(view);
    else if (name === "lsp.rename") startRename(view);
  };

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
    trackView,
    tooltip,
    linter(null),
    lintGutter(),
    diagnosticHoverExtension(),
    keys,
    clickToDefine,
    modifierHint,
    lspCompletionExtension(bridge),
    lspPanelExtension(panelBridge),
  ];
}

function diagnosticAt(view: EditorView, pos: number): EditorLspDiagnostic | null {
  let found: EditorLspDiagnostic | null = null;
  forEachDiagnostic(view.state, (diagnostic, from, to) => {
    if (found || pos < from || pos > to) return;
    const { line, col } = positionAt(view.state.doc, from);
    found = {
      file: "",
      lnum: line,
      col,
      message: diagnostic.message,
      severity: diagnostic.severity,
      source: "",
    };
  });
  if (found) return found;
  const line = view.state.doc.lineAt(pos).number;
  forEachDiagnostic(view.state, (diagnostic, from) => {
    if (found || view.state.doc.lineAt(from).number !== line) return;
    const position = positionAt(view.state.doc, from);
    found = {
      file: "",
      lnum: position.line,
      col: position.col,
      message: diagnostic.message,
      severity: diagnostic.severity,
      source: "",
    };
  });
  return found;
}

function diagnosticTooltip(pos: number, diagnostic: EditorLspDiagnostic): Tooltip {
  const text = diagnostic.source ? `${diagnostic.message}\n\n${diagnostic.source}` : diagnostic.message;
  return { pos, above: true, create: () => ({ dom: markdownElement(text, "cm-lsp-diagnostic") }) };
}
