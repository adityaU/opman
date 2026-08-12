/**
 * A Neovim-bound editor answers every key it receives, so the app keymap must
 * stand down while one holds focus. The claim is a DOM attribute rather than
 * React context because both listeners need to inspect the event target at
 * document-level dispatch time.
 */
export const NVIM_CAPTURE_ATTRIBUTE = "data-nvim-capture";

/**
 * Inputs and textareas are excluded because fields inside the editor—the LSP
 * rename field and search panels—belong to the app.
 */
export function nvimOwnsKey(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return false;
  return target.closest(`[${NVIM_CAPTURE_ATTRIBUTE}]`) !== null;
}
