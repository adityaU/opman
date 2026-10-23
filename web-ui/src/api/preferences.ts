import { apiFetch, apiPut } from "./client";
import type { Appearance } from "../utils/appearance";
import type { ThemeMode } from "../theme-selector/persistence";
import type { Density } from "../utils/density";

export interface Preferences {
  readonly appearance: Appearance | null;
  readonly themeMode: ThemeMode | null;
  readonly density: Density | null;
}

export const PREFERENCES_CHANGED = "opman:preferences-changed";
export const fetchPreferences = () => apiFetch<Preferences>("/preferences");
export const savePreferences = (patch: Partial<Preferences>) => apiPut<Preferences>("/preferences", patch);

export function parsePreferences(value: unknown): Partial<Preferences> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  const data = value as Record<string, unknown>;
  return {
    appearance: data.appearance === "light" || data.appearance === "dark" || data.appearance === "system"
      ? data.appearance : null,
    themeMode: data.themeMode === "flat" || data.themeMode === "glassy" ? data.themeMode : null,
    density: data.density === "compact" || data.density === "default" || data.density === "roomy"
      ? data.density : null,
  };
}

export function publishPreferences(value: unknown): void {
  window.dispatchEvent(new CustomEvent(PREFERENCES_CHANGED, { detail: parsePreferences(value) }));
}
