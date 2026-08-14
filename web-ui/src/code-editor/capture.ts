/**
 * The app keymap listens in the capture phase, so it sees a keystroke before
 * CodeMirror does. That is right for most keys — a global chord should beat a
 * text field — but wrong while the editor has an overlay open: `escape` means
 * "close this completion list", not "close the panel behind it", and the arrow
 * keys are steering the list rather than the app.
 *
 * Neovim used to make the same claim through `data-nvim-capture`. With Neovim
 * gone the editor still needs one, but only for as long as an overlay is up.
 */

/** Keys an open completion list or search panel is steering. */
const OVERLAY_KEYS: ReadonlySet<string> = new Set([
  "Escape", "ArrowUp", "ArrowDown", "PageUp", "PageDown",
]);

const OVERLAY_SELECTOR = ".cm-tooltip-autocomplete, .cm-panel.cm-search";

export function editorOwnsKey(target: EventTarget | null, key: string): boolean {
  if (!OVERLAY_KEYS.has(key)) return false;
  if (!(target instanceof Element)) return false;
  const editor = target.closest(".cm-editor");
  if (!editor) return false;
  // Tooltips are portalled next to the editor rather than inside it, so the
  // search is rooted at the panel the editor lives in.
  const root = editor.closest(".code-editor-panel") ?? editor;
  return root.querySelector(OVERLAY_SELECTOR) !== null;
}
