import { foldAll, foldCode, foldGutter, foldService, language, unfoldAll, unfoldCode } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import type { EditorView, ViewUpdate } from "@codemirror/view";
import { ViewPlugin, type PluginValue } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

type FoldCommand = (view: EditorView) => boolean;

function foldMarker(open: boolean): HTMLElement {
  const marker = document.createElement("span");
  marker.classList.add("cm-opman-fold-marker");
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", open ? "m6 9 6 6 6-6" : "m9 18 6-6-6-6");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-linecap", "round");
  path.setAttribute("stroke-linejoin", "round");
  path.setAttribute("stroke-width", "2");
  svg.append(path);
  marker.append(svg);
  return marker;
}

function isNvimNormalMode(view: EditorView): boolean {
  const mode = view.dom.querySelector<HTMLElement>(".cm-nvim-mode-label")?.dataset.mode;
  if (!mode) return false;
  return !mode.startsWith("insert") && !mode.startsWith("replace") && mode !== "terminal";
}

function runFoldCommand(view: EditorView, command: FoldCommand): void {
  command(view);
}

/**
 * Plain-text buffers have no syntax tree, but Neovim still permits folding
 * delimiter blocks. Keep the local gutter useful for those buffers too; a
 * language extension remains the source of truth whenever one is present.
 */
export function plainTextFold(state: EditorState, lineStart: number, _lineEnd: number): { from: number; to: number } | null {
  if (state.facet(language) !== null) return null;
  const opener = /[\[{]/;
  const closer: Record<string, string> = { "(": ")", "[": "]", "{": "}" };
  const line = state.doc.lineAt(lineStart);
  const start = line.text.search(opener);
  if (start < 0) return null;

  const open = line.text[start];
  const close = closer[open];
  let depth = 0;
  for (let offset = line.from + start; offset < state.doc.length; offset += 1) {
    const character = state.doc.sliceString(offset, offset + 1);
    if (character === open) depth += 1;
    else if (character === close) {
      depth -= 1;
      if (depth === 0 && offset > line.to) return { from: line.from + start + 1, to: offset };
    }
  }
  return null;
}

/**
 * Mirrors the fold commands Neovim receives into CodeMirror's local fold
 * state. The window capture listener runs before the edit binding's document
 * listener, so Neovim still receives every key in the sequence.
 */
class NvimFoldMirror implements PluginValue {
  private pendingPrefix = false;
  private readonly observer: MutationObserver;

  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (!(event.target instanceof Node) || !this.view.dom.contains(event.target)) return;
    if (!isNvimNormalMode(this.view) && this.view.dom.querySelector(".cm-nvim-status-panel")) {
      this.pendingPrefix = false;
      return;
    }

    if (event.key === "Escape") {
      this.pendingPrefix = false;
      return;
    }

    if (this.pendingPrefix) {
      this.pendingPrefix = false;
      const commands: Readonly<Record<string, FoldCommand>> = {
        c: foldCode,
        o: unfoldCode,
        R: unfoldAll,
        M: foldAll,
      };
      const command = commands[event.key];
      if (command) runFoldCommand(this.view, command);
      return;
    }

    this.pendingPrefix = event.key === "z";
  };

  constructor(private readonly view: EditorView) {
    window.addEventListener("keydown", this.onKeyDown, true);
    this.observer = new MutationObserver(() => this.revealGutterSpacer());
    this.observer.observe(view.dom, { subtree: true, childList: true, attributes: true, attributeFilter: ["style"] });
    this.revealGutterSpacer();
  }

  update(_update: ViewUpdate): void {
    // The editor view is intentionally read at event time; mode changes arrive
    // asynchronously from Neovim and should not rebuild this extension.
    this.revealGutterSpacer();
  }

  destroy(): void {
    window.removeEventListener("keydown", this.onKeyDown, true);
    this.observer.disconnect();
  }

  private revealGutterSpacer(): void {
    const spacers = this.view.dom.querySelectorAll<HTMLElement>(".cm-foldGutter .cm-gutterElement");
    for (const spacer of spacers) {
      if (spacer.style.visibility === "hidden") spacer.style.removeProperty("visibility");
    }
  }
}

const opmanFoldGutter = foldGutter({ markerDOM: foldMarker });

export const nvimFoldMirrorExtension: Extension = [
  opmanFoldGutter,
  foldService.of(plainTextFold),
  ViewPlugin.fromClass(NvimFoldMirror),
];
