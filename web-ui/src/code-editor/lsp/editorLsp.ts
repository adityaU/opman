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
import { keymap, EditorView, type Tooltip } from "@codemirror/view";
import { forEachDiagnostic, lintGutter, linter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import type { EditorLspDiagnostic } from "../types";
import type {
  EditorDefinitionLocation, EditorGotoKind, EditorHoverResponse, EditorLspActions,
  EditorReferenceLocation,
} from "../../api";
import { markdownElement } from "./lspMarkdown";
import { actionLabel, hoverCard, type HoverActionRequest } from "./hoverCard";
import { holdingHoverTooltip } from "./hoverHold";
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
  hoverAt: (line: number, col: number) => Promise<EditorHoverResponse | null>;
  definitionAt: (line: number, col: number) => Promise<boolean>;
  /** Resolve one of the four navigations without moving the reader. */
  gotoAt: (
    kind: EditorGotoKind,
    line: number,
    col: number,
  ) => Promise<readonly EditorDefinitionLocation[]>;
  /** Open a resolved location, either in place or in a pane the user picks. */
  reveal: (file: string, line: number, where: "here" | "choose", label: string) => void;
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

export function createLspExtensions(bridge: LspBridgeRef) {
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

  /**
   * A hover card action. Navigations resolve first and only then decide what to
   * do with the answer: one location goes straight there, several open the same
   * list the references panel uses, because "go to definition" with three
   * candidates is a choice, not a jump.
   */
  const runHoverAction = (
    view: EditorView,
    line: number,
    col: number,
    symbol: string,
    request: HoverActionRequest,
  ): void => {
    if (request.action === "rename") {
      view.dispatch({ selection: { anchor: offsetAt(view.state.doc, line, col) } });
      startRename(view);
      return;
    }
    if (request.action === "references") {
      view.dispatch({ selection: { anchor: offsetAt(view.state.doc, line, col) } });
      showReferences(view);
      return;
    }
    const label = actionLabel(request.action, symbol);
    void bridge.current.gotoAt(request.action, line, col).then((locations) => {
      if (locations.length === 0) return;
      if (locations.length === 1) {
        const [only] = locations;
        bridge.current.reveal(only.file, only.lnum, request.where, label);
        return;
      }
      openLspPanel(view, {
        kind: "references",
        locations: locations.map((location) => ({ ...location, text: location.file })),
        busy: false,
      });
    });
  };

  // Held open for five seconds after the pointer leaves — see `hoverHold`. The
  // card has buttons in it, and CodeMirror's own hover drops it the instant the
  // pointer moves toward one.
  const tooltip = holdingHoverTooltip(async (view, pos): Promise<Tooltip | null> => {
    const diagnostic = diagnosticAt(view, pos);
    if (diagnostic) return diagnosticTooltip(pos, diagnostic);
    if (!bridge.current.enabled) return null;
    const { line, col } = positionAt(view.state.doc, pos);
    const response = await bridge.current.hoverAt(line, col);
    if (!response?.hover) return null;
    const actions = response.actions ?? NO_ACTIONS;
    const symbol = wordAt(view, pos);
    return {
      pos,
      above: true,
      // Servers answer in markdown — a fenced signature block, then prose.
      // Printed literally that is a wall of backticks.
      create: () => ({
        dom: hoverCard({
          text: response.hover ?? "",
          actions,
          onAction: (request) => runHoverAction(view, line, col, symbol, request),
        }),
      }),
    };
  });

  // The editor's own shortcuts for definition, references, rename and format.
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

/** An older backend answers a hover without saying what else it can do. */
const NO_ACTIONS: EditorLspActions = {
  definition: true, typeDefinition: false, implementation: false, declaration: false,
  references: true, rename: true, format: true,
};

/** The identifier under a position, for naming what the pane chooser is placing. */
function wordAt(view: EditorView, pos: number): string {
  const line = view.state.doc.lineAt(pos);
  const offset = pos - line.from;
  const word = /[A-Za-z_$][\w$]*/g;
  for (let match = word.exec(line.text); match; match = word.exec(line.text)) {
    if (match.index <= offset && offset <= match.index + match[0].length) return match[0];
  }
  return "";
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
