/** The terminal font size, remembered across panes and reloads. */

const KEY = "opman.terminal.fontSize";
export const DEFAULT_FONT_SIZE = 13;
export const MIN_FONT_SIZE = 8;
export const MAX_FONT_SIZE = 32;
export const FONT_CHANGED_EVENT = "opman:terminal-font-size";

export function clampFontSize(size: number): number {
  if (!Number.isFinite(size)) return DEFAULT_FONT_SIZE;
  return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, Math.round(size)));
}

export function loadFontSize(): number {
  try {
    const raw = localStorage.getItem(KEY);
    return raw === null ? DEFAULT_FONT_SIZE : clampFontSize(Number(raw));
  } catch {
    return DEFAULT_FONT_SIZE;
  }
}

/** Save the size and tell every open terminal. */
export function saveFontSize(size: number): number {
  const next = clampFontSize(size);
  try {
    localStorage.setItem(KEY, String(next));
  } catch {
    /* private mode: the size still applies to this page */
  }
  window.dispatchEvent(new CustomEvent(FONT_CHANGED_EVENT, { detail: next }));
  return next;
}

/** One step bigger or smaller, or back to the default when `step` is 0. */
export function stepFontSize(step: -1 | 0 | 1): number {
  return saveFontSize(step === 0 ? DEFAULT_FONT_SIZE : loadFontSize() + step);
}
