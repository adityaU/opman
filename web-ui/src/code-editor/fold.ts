import { foldGutter, foldService, language } from "@codemirror/language";
import type { EditorState, Extension } from "@codemirror/state";

/**
 * Code folding for the editor.
 *
 * CodeMirror's own `foldGutter` does the work; this module only supplies the
 * chevron marker the rest of the UI draws with, and keeps the gutter useful in
 * buffers that have no syntax tree. `foldKeymap` arrives with `basicSetup`.
 */

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

/**
 * Plain-text buffers have no syntax tree and so no folds at all. Matching
 * delimiter blocks keeps the gutter useful for them; a language extension
 * remains the source of truth whenever one is present.
 */
export function plainTextFold(
  state: EditorState,
  lineStart: number,
  _lineEnd: number,
): { from: number; to: number } | null {
  if (state.facet(language) !== null) return null;
  const opener = /[[{]/;
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
 * The gutter's width is held by a `min-width` in `editor-folds.css` rather than
 * by revealing CodeMirror's hidden spacer rows. Un-hiding them used to lay a
 * full-height transparent element over the chevrons, so the gutter itself won
 * every click, and it cost a subtree MutationObserver per editor to do it.
 */
export const foldGutterExtension: Extension = [
  foldGutter({ markerDOM: foldMarker }),
  foldService.of(plainTextFold),
];
