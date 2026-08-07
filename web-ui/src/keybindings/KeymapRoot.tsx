import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import {
  KEYBINDINGS_CHANGED,
  loadKeybindingsOrDefault,
  type KeybindingsChanged,
} from "../api/keybindings";
import { DEFAULT_CONFIG } from "./config";
import type { KeybindingsConfig } from "./config";
import { KeymapProvider } from "./KeymapContext";
import { useDirectionalNav } from "./useDirectionalNav";
import { useKeymapListener } from "./useKeymapListener";
import { useSurfaceFocus } from "./useSurfaceFocus";
import { PendingChordStrip, WhichKeyPanel } from "./which-key/WhichKeyPanel";

/**
 * Mounts the keymap for the whole app.
 *
 * Renders immediately on the defaults and swaps in the user's config when it
 * arrives, rather than blocking paint: a keyboard that works one render late is
 * better than an app that will not draw because a config fetch is slow.
 */

export function KeymapRoot({ children }: { readonly children: ReactNode }) {
  const [config, setConfig] = useState<KeybindingsConfig>(DEFAULT_CONFIG);

  useEffect(() => {
    let cancelled = false;
    loadKeybindingsOrDefault().then((response) => {
      if (!cancelled) setConfig(response.config);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // The mount fetch is the only other way this is set, so an edit made in the keybindings
  // view would otherwise not reach the live keymap until the page was reloaded — the
  // keyboard would keep running the bindings, and the mode, the app started with.
  useEffect(() => {
    const apply = (event: Event) => setConfig((event as KeybindingsChanged).detail);
    window.addEventListener(KEYBINDINGS_CHANGED, apply);
    return () => window.removeEventListener(KEYBINDINGS_CHANGED, apply);
  }, []);

  return (
    <KeymapProvider config={config}>
      {children}
      <KeymapSurface />
    </KeymapProvider>
  );
}

/**
 * The listener and its hint surfaces.
 *
 * Separate from the provider so that the pending-chord state re-renders only
 * these two components, not the whole app on every keystroke of a chord.
 */
function KeymapSurface() {
  const listener = useKeymapListener();
  useSurfaceFocus();
  useDirectionalNav();
  return (
    <>
      <WhichKeyPanel listener={listener} />
      <PendingChordStrip listener={listener} />
    </>
  );
}
