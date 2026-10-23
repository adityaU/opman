import { useEffect } from "react";
import { fetchPreferences, PREFERENCES_CHANGED, parsePreferences } from "../api/preferences";
import { setAppearance, type Appearance } from "../utils/appearance";
import { applyThemeMode, persistThemeMode, type ThemeMode } from "../theme-selector/persistence";
import { applyDensity, persistDensity } from "../utils/density";

/** Apply AI and remote-device settings to this browser without opening Settings. */
export function useServerPreferences(
  onAppearance: (value: Appearance) => void,
  onThemeMode: (value: ThemeMode) => void,
) {
  useEffect(() => {
    let active = true;
    let revision = 0;
    const apply = (value: unknown) => {
      if (!active) return;
      const prefs = parsePreferences(value);
      if (prefs.appearance) {
        setAppearance(prefs.appearance);
        onAppearance(prefs.appearance);
      }
      if (prefs.themeMode) {
        persistThemeMode(prefs.themeMode);
        applyThemeMode(prefs.themeMode);
        onThemeMode(prefs.themeMode);
      }
      if (prefs.density) {
        persistDensity(prefs.density);
        applyDensity(prefs.density);
      }
    };
    const changed = (event: Event) => {
      revision++;
      apply((event as CustomEvent).detail);
    };
    window.addEventListener(PREFERENCES_CHANGED, changed);
    const requestedAt = revision;
    void fetchPreferences().then(value => {
      if (revision === requestedAt) apply(value);
    }).catch(() => { /* Keep local choices when connected to an older server. */ });
    return () => {
      active = false;
      window.removeEventListener(PREFERENCES_CHANGED, changed);
    };
  }, [onAppearance, onThemeMode]);
}
