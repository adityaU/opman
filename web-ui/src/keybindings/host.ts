import type { Browser, Host, Platform, ReservedChord, Target } from "./types";

/**
 * Host detection and the table of chords the host takes before the page sees
 * them. Only chords that cannot be cancelled from a keydown handler belong
 * here — Ctrl+P, Ctrl+S and Ctrl+F are all preventable and so keep their
 * VSCode meaning.
 *
 * The desktop target has no entries at all, which is the point: the base
 * keymap is the canonical one and the web layer is what bends around a browser.
 */

export function detectPlatform(nav: Pick<Navigator, "userAgent" | "platform"> = navigator): Platform {
  const ua = `${nav.platform ?? ""} ${nav.userAgent ?? ""}`.toLowerCase();
  if (ua.includes("mac") || ua.includes("iphone") || ua.includes("ipad")) return "mac";
  if (ua.includes("win")) return "win";
  return "linux";
}

export function detectBrowser(userAgent: string = navigator.userAgent): Browser {
  const ua = userAgent.toLowerCase();
  if (ua.includes("firefox")) return "firefox";
  if (ua.includes("edg/") || ua.includes("chrome") || ua.includes("chromium")) return "chrome";
  if (ua.includes("safari")) return "safari";
  return "other";
}

/**
 * A standalone display mode means an installed PWA or the desktop shell, where
 * the browser chrome — and its shortcuts — are gone.
 */
export function detectTarget(): Target {
  if (typeof window === "undefined") return "web";
  const nativeShell = "__OPMAN_DESKTOP__" in window;
  if (nativeShell) return "desktop";
  const standalone = window.matchMedia?.("(display-mode: standalone)")?.matches ?? false;
  return standalone ? "desktop" : "web";
}

export function detectHost(): Host {
  return { platform: detectPlatform(), target: detectTarget(), browser: detectBrowser() };
}

/** Chords every browser takes on every platform. */
const UNIVERSAL: readonly ReservedChord[] = [
  { id: "meta+n", owner: "new browser window" },
  { id: "meta+t", owner: "new browser tab" },
  { id: "meta+w", owner: "close browser tab" },
  { id: "shift+meta+n", owner: "new incognito window" },
  { id: "shift+meta+t", owner: "reopen closed tab" },
  { id: "shift+meta+w", owner: "close browser window" },
  ...digits("meta", "switch browser tab"),
];

const FIREFOX: readonly ReservedChord[] = [
  { id: "shift+meta+p", owner: "Firefox private window" },
  { id: "shift+meta+k", owner: "Firefox web console" },
  { id: "shift+meta+e", owner: "Firefox network monitor" },
  { id: "shift+meta+o", owner: "Firefox library" },
  { id: "shift+meta+s", owner: "Firefox screenshot" },
  { id: "shift+meta+i", owner: "Firefox devtools" },
  { id: "meta+q", owner: "quit Firefox" },
];

const CHROME: readonly ReservedChord[] = [
  { id: "shift+meta+i", owner: "Chrome devtools" },
  { id: "shift+meta+j", owner: "Chrome console" },
  { id: "shift+meta+c", owner: "Chrome inspect element" },
];

const SAFARI: readonly ReservedChord[] = [
  { id: "shift+meta+i", owner: "Safari web inspector" },
  { id: "alt+meta+i", owner: "Safari web inspector" },
];

/** Reserved by macOS itself, regardless of browser. */
const MACOS: readonly ReservedChord[] = [
  { id: "meta+q", owner: "quit application" },
  { id: "meta+m", owner: "minimize window" },
  { id: "meta+h", owner: "hide application" },
  { id: "meta+space", owner: "Spotlight" },
  { id: "meta+`", owner: "cycle application windows" },
  { id: "meta+,", owner: "browser preferences" },
];

const LINUX_WIN: readonly ReservedChord[] = [{ id: "ctrl+q", owner: "quit browser" }];

function digits(mod: string, owner: string): ReservedChord[] {
  return Array.from({ length: 9 }, (_, i) => ({ id: `${mod}+${i + 1}`, owner }));
}

/**
 * On macOS the browser shortcuts above use Command; elsewhere they use Control.
 * The table is authored once with `meta` and rewritten for the platform.
 */
function forPlatform(chords: readonly ReservedChord[], platform: Platform): ReservedChord[] {
  if (platform === "mac") return [...chords];
  return chords.map((c) => ({ ...c, id: c.id.replace(/\bmeta\+/g, "ctrl+") }));
}

const BY_BROWSER: Readonly<Record<Browser, readonly ReservedChord[]>> = {
  firefox: FIREFOX,
  chrome: CHROME,
  safari: SAFARI,
  other: [],
};

/** Every chord `host` will not deliver to the page, as normalized chord ids. */
export function reservedChords(host: Host): ReservedChord[] {
  if (host.target === "desktop") return [];
  const browser = forPlatform([...UNIVERSAL, ...BY_BROWSER[host.browser]], host.platform);
  const os = host.platform === "mac" ? MACOS : LINUX_WIN;
  return [...browser, ...os];
}

export function reservedChordIds(host: Host): ReadonlySet<string> {
  return new Set(reservedChords(host).map((c) => c.id));
}

export function reservedOwner(host: Host, chordId: string): string | undefined {
  return reservedChords(host).find((c) => c.id === chordId)?.owner;
}
