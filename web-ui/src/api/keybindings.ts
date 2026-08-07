import type { KeybindingsConfig, KeybindingsResponse } from "../keybindings/config";
import { DEFAULT_CONFIG, parseConfig } from "../keybindings/config";
import { apiFetch } from "./client";

/**
 * `~/.config/opman/keybindings.json`, read and written through the backend.
 *
 * The response carries diagnostics alongside the config: a malformed file
 * degrades to defaults on the server and the reason travels with it, so the
 * keybindings view can show what was ignored instead of the user finding out
 * by pressing a key that does nothing.
 */

/**
 * Window event carrying a config the user just committed.
 *
 * The keybindings view is not the only holder of this document — `KeymapRoot` owns the
 * copy the whole app's keyboard is composed from, and it fetches once at mount. Without
 * this the two drift the moment anything is edited: the file and the view agree, the live
 * keymap does not, and the view's own mode switch appears not to have worked because the
 * mode it renders comes from the app-level copy.
 *
 * The committed config travels in `detail` rather than the listener refetching, so the
 * keymap recomposes in the same tick the user clicked — matching the view's optimistic
 * write instead of trailing it by a round trip.
 */
export const KEYBINDINGS_CHANGED = "opman:keybindings-changed";

export type KeybindingsChanged = CustomEvent<KeybindingsConfig>;

/** Tell every other holder of the config what was just written. */
export function publishKeybindings(config: KeybindingsConfig): void {
  window.dispatchEvent(new CustomEvent(KEYBINDINGS_CHANGED, { detail: config }));
}

export async function fetchKeybindings(): Promise<KeybindingsResponse> {
  const raw = await apiFetch<KeybindingsResponse>("/keybindings");
  const parsed = parseConfig(raw?.config);
  return {
    config: parsed.config,
    diagnostics: [...(raw?.diagnostics ?? []), ...parsed.diagnostics],
    path: raw?.path ?? null,
  };
}

export async function saveKeybindings(
  config: KeybindingsConfig,
): Promise<KeybindingsResponse> {
  const raw = await apiFetch<KeybindingsResponse>("/keybindings", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  const parsed = parseConfig(raw?.config);
  return {
    config: parsed.config,
    diagnostics: [...(raw?.diagnostics ?? []), ...parsed.diagnostics],
    path: raw?.path ?? null,
  };
}

/**
 * Load the config, falling back to defaults when the request fails.
 *
 * Keybindings must work offline and before the backend is reachable — an app
 * whose keyboard stops working because a fetch failed is worse than one running
 * the defaults.
 */
export async function loadKeybindingsOrDefault(): Promise<KeybindingsResponse> {
  try {
    return await fetchKeybindings();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      config: DEFAULT_CONFIG,
      diagnostics: [{ message: `could not load keybindings.json: ${message}` }],
      path: null,
    };
  }
}
