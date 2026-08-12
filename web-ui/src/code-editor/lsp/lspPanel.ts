/**
 * The two LSP answers that do not fit in a tooltip: a list of references, and
 * the name a rename should introduce.
 *
 * Both live in a CodeMirror panel under the document rather than a modal, so
 * the code they are about stays on screen while you read or type.
 */
import { EditorView, showPanel, type Panel } from "@codemirror/view";
import { StateEffect, StateField, type Extension } from "@codemirror/state";
import type { EditorReferenceLocation } from "../../api";

export type LspPanelState =
  | { readonly kind: "none" }
  | { readonly kind: "references"; readonly locations: readonly EditorReferenceLocation[]; readonly busy: boolean }
  | { readonly kind: "rename"; readonly name: string; readonly busy: boolean };

export interface LspPanelBridge {
  readonly jumpTo: (file: string, line: number) => void;
  readonly applyRename: (name: string) => void;
}

export type LspPanelBridgeRef = { current: LspPanelBridge };

const CLOSED: LspPanelState = { kind: "none" };

export const lspPanelEffect = StateEffect.define<LspPanelState>();

export const lspPanelField = StateField.define<LspPanelState>({
  create: () => CLOSED,
  update(value, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(lspPanelEffect)) return effect.value;
    }
    return value;
  },
});

export function closeLspPanel(view: EditorView): boolean {
  if (view.state.field(lspPanelField).kind === "none") return false;
  view.dispatch({ effects: lspPanelEffect.of(CLOSED) });
  view.focus();
  return true;
}

export function openLspPanel(view: EditorView, state: LspPanelState): void {
  view.dispatch({ effects: lspPanelEffect.of(state) });
}

/** The identifier the caret sits in, which is what a rename starts from. */
export function wordAtCursor(view: EditorView): string {
  const { head } = view.state.selection.main;
  const line = view.state.doc.lineAt(head);
  const offset = head - line.from;
  const before = /[\w$]*$/.exec(line.text.slice(0, offset))?.[0] ?? "";
  const after = /^[\w$]*/.exec(line.text.slice(offset))?.[0] ?? "";
  return `${before}${after}`;
}

function element(tag: string, className: string, text?: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function baseName(path: string): string {
  const index = path.lastIndexOf("/");
  return index < 0 ? path : path.slice(index + 1);
}

function references(
  state: Extract<LspPanelState, { kind: "references" }>,
  bridge: LspPanelBridgeRef,
  view: EditorView,
): HTMLElement {
  const list = element("div", "cm-lsp-locations");
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", "References");
  if (state.busy) {
    list.append(element("div", "cm-lsp-panel-note", "Finding references…"));
    return list;
  }
  if (state.locations.length === 0) {
    list.append(element("div", "cm-lsp-panel-note", "No references found"));
    return list;
  }
  for (const location of state.locations) {
    const row = element("button", "cm-lsp-location");
    row.setAttribute("type", "button");
    row.setAttribute("role", "option");
    row.append(
      element("span", "cm-lsp-location-where", `${baseName(location.file)}:${location.lnum}`),
      element("span", "cm-lsp-location-text", location.text),
    );
    row.addEventListener("click", () => {
      bridge.current.jumpTo(location.file, location.lnum);
      closeLspPanel(view);
    });
    list.append(row);
  }
  return list;
}

function rename(
  state: Extract<LspPanelState, { kind: "rename" }>,
  bridge: LspPanelBridgeRef,
  view: EditorView,
): HTMLElement {
  const box = element("div", "cm-lsp-rename");
  const label = element("label", "cm-lsp-panel-note", state.busy ? "Renaming…" : "Rename to");
  const input = document.createElement("input");
  input.className = "cm-lsp-rename-input";
  input.value = state.name;
  input.disabled = state.busy;
  input.setAttribute("aria-label", "New symbol name");
  label.setAttribute("for", "cm-lsp-rename-input");
  input.id = "cm-lsp-rename-input";
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      bridge.current.applyRename(input.value);
      return;
    }
    if (event.key !== "Escape") return;
    event.preventDefault();
    closeLspPanel(view);
  });
  box.append(label, input);
  queueMicrotask(() => {
    if (!state.busy) input.select();
  });
  return box;
}

function render(dom: HTMLElement, state: LspPanelState, bridge: LspPanelBridgeRef, view: EditorView): void {
  dom.replaceChildren(
    ...(state.kind === "references"
      ? [references(state, bridge, view)]
      : state.kind === "rename"
        ? [rename(state, bridge, view)]
        : []),
  );
  dom.hidden = state.kind === "none";
}

export function lspPanelExtension(bridge: LspPanelBridgeRef): Extension {
  const panel = (view: EditorView): Panel => {
    const dom = element("div", "cm-lsp-panel");
    render(dom, view.state.field(lspPanelField), bridge, view);
    // Neovim's binding claims keydown in the capture phase, so an Escape meant
    // for this panel would otherwise never reach CodeMirror's keymap.
    const dismiss = (event: KeyboardEvent): void => {
      if (event.key === "Escape") closeLspPanel(view);
    };
    document.addEventListener("keydown", dismiss, true);
    return {
      dom,
      destroy: () => document.removeEventListener("keydown", dismiss, true),
      update(update) {
        const next = update.state.field(lspPanelField);
        if (next !== update.startState.field(lspPanelField)) render(dom, next, bridge, update.view);
      },
    };
  };
  return [
    lspPanelField,
    showPanel.of(panel),
    EditorView.baseTheme({
      ".cm-lsp-panel": {
        borderTop: "1px solid var(--color-border-subtle)",
        backgroundColor: "var(--color-bg-panel)",
        padding: "4px 8px",
        fontSize: "12px",
      },
      ".cm-lsp-panel[hidden]": { display: "none" },
      ".cm-lsp-panel-note": { color: "var(--color-text-muted)", marginInlineEnd: "6px" },
      ".cm-lsp-locations": { display: "flex", flexDirection: "column", maxHeight: "150px", overflowY: "auto" },
      ".cm-lsp-location": {
        display: "flex",
        gap: "8px",
        padding: "2px 4px",
        border: "none",
        borderRadius: "var(--radius, 6px)",
        background: "transparent",
        color: "var(--color-text)",
        font: "inherit",
        textAlign: "left",
        cursor: "pointer",
      },
      ".cm-lsp-location:hover": { backgroundColor: "var(--color-bg-hover, rgba(127,127,127,0.15))" },
      ".cm-lsp-location-where": { color: "var(--color-primary)", flexShrink: "0" },
      ".cm-lsp-location-text": { color: "var(--color-text-muted)", overflow: "hidden", whiteSpace: "nowrap", textOverflow: "ellipsis" },
      ".cm-lsp-rename": { display: "flex", alignItems: "center", gap: "6px" },
      ".cm-lsp-rename-input": {
        flex: "1",
        padding: "2px 6px",
        border: "1px solid var(--color-border-subtle)",
        borderRadius: "var(--radius, 6px)",
        background: "var(--color-bg)",
        color: "var(--color-text)",
        font: "inherit",
      },
    }),
  ];
}
