/**
 * The two client-side theme axes: surface treatment and appearance.
 *
 * The palette itself lives server-side (the opencode KV store), which is why
 * only these two are in localStorage.
 */
export type ThemeMode = "glassy" | "flat";

const THEME_MODE_KEY = "opman-theme-mode";

/** Read persisted theme mode from localStorage. */
export function getPersistedThemeMode(): ThemeMode {
  try {
    const v = localStorage.getItem(THEME_MODE_KEY);
    if (v === "glassy") return "glassy";
  } catch { /* ignore */ }
  return "flat";
}

/** Persist theme mode to localStorage. */
export function persistThemeMode(mode: ThemeMode): void {
  try { localStorage.setItem(THEME_MODE_KEY, mode); } catch { /* ignore */ }
}

/** Apply or remove the flat-theme class on <html>. */
export function applyThemeMode(mode: ThemeMode): void {
  document.documentElement.classList.toggle("flat-theme", mode === "flat");
}
