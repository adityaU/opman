import type { ThemePair, ThemeColors } from "../api";
import { applyThemeToCss, notifyThemeChanged } from "./theme";

// ── Types ────────────────────────────────────────────────────────────

export type Appearance = "system" | "light" | "dark";

// ── localStorage ─────────────────────────────────────────────────────

const APPEARANCE_KEY = "opman-appearance";

export function getPersistedAppearance(): Appearance {
  try {
    const v = localStorage.getItem(APPEARANCE_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch { /* ignore */ }
  return "dark";
}

export function persistAppearance(a: Appearance): void {
  try { localStorage.setItem(APPEARANCE_KEY, a); } catch { /* ignore */ }
}

// ── Resolve effective appearance ─────────────────────────────────────

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Resolve "system" to either "dark" or "light". */
export function resolveAppearance(a: Appearance): "dark" | "light" {
  if (a === "system") return systemPrefersDark() ? "dark" : "light";
  return a;
}

// ── Apply CSS class on <html> ────────────────────────────────────────

export function applyAppearanceClass(a: Appearance): void {
  const resolved = resolveAppearance(a);
  const root = document.documentElement;
  if (resolved === "light") {
    root.classList.add("light-theme");
  } else {
    root.classList.remove("light-theme");
  }
  root.setAttribute("data-appearance", resolved);
  // Light/dark can flip without any colour variable changing (the class alone
  // decides which end of the ink scale a terminal should use).
  notifyThemeChanged();
}

// ── ThemePair resolution ─────────────────────────────────────────────

/** Pick the correct variant from a ThemePair based on appearance. */
export function resolveThemeColors(pair: ThemePair, a: Appearance): ThemeColors {
  return resolveAppearance(a) === "light" ? pair.light : pair.dark;
}

// ── Stored ThemePair for OS listener ─────────────────────────────────

let _storedPair: ThemePair | null = null;
let _storedAppearance: Appearance = "dark";
let _cleanupListener: (() => void) | null = null;

/** Store the current ThemePair so the OS listener can re-apply. */
export function storeThemePair(pair: ThemePair, appearance: Appearance): void {
  _storedPair = pair;
  _storedAppearance = appearance;
}

/**
 * Name of the palette currently applied, or "" before the first pair arrives.
 *
 * The active theme lives server-side, so this is the only thing the client can
 * compare a theme list against — a picker without it has to guess, and guessing
 * means highlighting row zero.
 */
export function activeThemeName(): string {
  return _storedPair?.name ?? "";
}

/**
 * Install a matchMedia listener that re-applies the theme when the OS
 * dark/light preference changes. Only triggers when appearance = "system".
 * Call once on app init. Returns a cleanup function.
 */
export function installSystemListener(): () => void {
  if (_cleanupListener) _cleanupListener();

  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => {
    if (_storedAppearance !== "system" || !_storedPair) return;
    applyAppearanceClass("system");
    applyThemeToCss(resolveThemeColors(_storedPair, "system"));
  };

  mq.addEventListener("change", handler);
  _cleanupListener = () => mq.removeEventListener("change", handler);
  return _cleanupListener;
}

/**
 * Full appearance init: read localStorage, apply CSS class, install
 * OS listener. Call once on app mount (before theme fetch).
 */
export function initAppearance(): Appearance {
  const a = getPersistedAppearance();
  applyAppearanceClass(a);
  installSystemListener();
  return a;
}

/**
 * Set appearance: persist, apply class, update stored ref, re-apply
 * theme colors if we have a stored ThemePair.
 */
export function setAppearance(a: Appearance): void {
  persistAppearance(a);
  applyAppearanceClass(a);
  _storedAppearance = a;
  if (_storedPair) {
    applyThemeToCss(resolveThemeColors(_storedPair, a));
  }
}
