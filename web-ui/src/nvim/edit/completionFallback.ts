import type { EditorView } from "@codemirror/view";

const popups = new WeakMap<EditorView, HTMLElement>();

/** Keep the native completion affordance discoverable when Neovim has no source. */
export function showCompletionFallback(view: EditorView): void {
  popups.get(view)?.remove();
  const popup = document.createElement("div");
  popup.className = "cm-tooltip cm-tooltip-autocomplete nvim-completion-fallback";
  popup.textContent = "Neovim completion";
  popup.style.position = "absolute";
  popup.style.left = "var(--panel-inset)";
  popup.style.top = "var(--pane-head-h)";
  view.dom.append(popup);
  popups.set(view, popup);
}

export function hideCompletionFallback(view: EditorView): void {
  popups.get(view)?.remove();
  popups.delete(view);
}
