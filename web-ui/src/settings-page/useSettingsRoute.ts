import { useCallback, useEffect, useState } from "react";
import { appNavigate, onLocationChange, SETTINGS_PATH } from "../utils/navigation";

/**
 * Path-based route for the settings page.
 *
 * Settings is a destination (`/settings`), not a modal: it holds five editors, each of
 * which owns unsaved text or an in-flight login, and none of that survives an Escape
 * keypress aimed at something else. Being a path also makes a section linkable — the
 * command palette and the keymap open `?section=keybindings` directly rather than opening
 * a surface and then navigating inside it.
 */

export { SETTINGS_PATH };

export const SETTINGS_SECTIONS = ["appearance", "keybindings", "acp", "mcp", "skills"] as const;

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];

const DEFAULT_SECTION: SettingsSection = "appearance";

export interface SettingsRoute {
  /** True when the current path is the settings page. */
  readonly isSettingsView: boolean;
  /** Section on screen. Always valid — an unknown `?section=` falls back rather than
   *  rendering nothing, because a stale bookmark should still open settings. */
  readonly section: SettingsSection;
  /** Navigate to a section, replacing the URL so back leaves settings in one step. */
  readonly openSection: (section: SettingsSection) => void;
}

function readView(): boolean {
  return window.location.pathname.startsWith(SETTINGS_PATH);
}

function readSection(): SettingsSection {
  const raw = new URLSearchParams(window.location.search).get("section");
  return SETTINGS_SECTIONS.find((name) => name === raw) ?? DEFAULT_SECTION;
}

/** Build a settings URL for a section. */
export function settingsUrl(section?: SettingsSection): string {
  return section && section !== DEFAULT_SECTION
    ? `${SETTINGS_PATH}?section=${section}`
    : SETTINGS_PATH;
}

/** Open settings from anywhere — the palette, the keymap, a status-bar button. */
export function openSettings(section?: SettingsSection): void {
  appNavigate(settingsUrl(section));
}

/**
 * Open a section, or leave settings when that section is already on screen.
 *
 * The chord that opens a surface is expected to close it again. Going *back* rather than
 * forward to the chat is what makes that true — it returns the user to the conversation
 * they were in, which no amount of URL construction here could know.
 */
export function toggleSettings(section?: SettingsSection): void {
  const showing = readView() && readSection() === (section ?? DEFAULT_SECTION);
  // A tab opened straight onto /settings has nothing to go back to; opening the section
  // again is a harmless no-op, where `back()` would leave the app.
  if (showing && window.history.length > 1) {
    window.history.back();
    return;
  }
  openSettings(section);
}

export function useSettingsRoute(): SettingsRoute {
  const [isSettingsView, setIsSettingsView] = useState<boolean>(readView);
  const [section, setSection] = useState<SettingsSection>(readSection);

  // Switching section is not a separate back-stop: one back gesture should return to the
  // chat the user came from, not walk them through every section they looked at.
  const openSection = useCallback((next: SettingsSection) => {
    appNavigate(settingsUrl(next), { replace: readView() });
  }, []);

  useEffect(
    () =>
      onLocationChange(() => {
        setIsSettingsView(readView());
        setSection(readSection());
      }),
    [],
  );

  return { isSettingsView, section, openSection };
}
