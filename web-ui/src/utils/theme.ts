import type { ThemeColors } from "../api";

/** Apply theme colors as CSS custom properties on :root */
export function applyThemeToCss(colors: ThemeColors) {
  const root = document.documentElement.style;
  root.setProperty("--color-primary", colors.primary);
  root.setProperty("--color-secondary", colors.secondary);
  root.setProperty("--color-accent", colors.accent);
  root.setProperty("--color-bg", colors.background);
  root.setProperty("--color-bg-panel", colors.background_panel);
  root.setProperty("--color-bg-element", colors.background_element);
  root.setProperty("--color-text", colors.text);
  root.setProperty("--color-text-muted", colors.text_muted);
  root.setProperty("--color-border", colors.border);
  root.setProperty("--color-border-active", colors.border_active);
  root.setProperty("--color-border-subtle", colors.border_subtle);
  root.setProperty("--color-error", colors.error);
  root.setProperty("--color-warning", colors.warning);
  root.setProperty("--color-success", colors.success);
  root.setProperty("--color-info", colors.info);

  // Sync browser / PWA status-bar chrome to the app background color
  syncMetaThemeColor(colors.background);

  // Sync favicon + PWA icons to match the theme's primary + background colors
  updateFavicon(colors.primary, colors.background);
  updatePwaIcons(colors.primary, colors.background);

  // Push theme colors to the service worker so it can intercept
  // manifest/icon requests and return themed versions (installed PWA).
  notifyServiceWorker(colors.primary, colors.background);

  // Semantic aliases (used widely in component CSS)
  root.setProperty("--color-surface", colors.background_panel);
  root.setProperty("--color-text-secondary", colors.text_muted);

  // Semantic aliases so UI surfaces avoid hardcoded/accent-specific assumptions
  root.setProperty("--theme-surface-1", colors.background_panel);
  root.setProperty("--theme-surface-2", colors.background_element);
  root.setProperty("--theme-surface-3", `color-mix(in srgb, ${colors.background_element} 76%, ${colors.background_panel})`);
  root.setProperty("--theme-surface-hover", `color-mix(in srgb, ${colors.background_element} 88%, ${colors.text} 12%)`);
  root.setProperty("--theme-elevated", `color-mix(in srgb, ${colors.background_panel} 86%, ${colors.text} 14%)`);
  root.setProperty("--theme-overlay", `color-mix(in srgb, ${colors.background} 72%, transparent)`);
  root.setProperty("--theme-focus-ring", `color-mix(in srgb, ${colors.primary} 18%, transparent)`);
  root.setProperty("--theme-primary-soft", `color-mix(in srgb, ${colors.primary} 10%, ${colors.background_element})`);
  root.setProperty("--theme-primary-border", `color-mix(in srgb, ${colors.primary} 24%, ${colors.border})`);
  root.setProperty("--theme-success-soft", `color-mix(in srgb, ${colors.success} 12%, ${colors.background_element})`);
  root.setProperty("--theme-error-soft", `color-mix(in srgb, ${colors.error} 12%, ${colors.background_element})`);
  root.setProperty("--theme-warning-soft", `color-mix(in srgb, ${colors.warning} 12%, ${colors.background_element})`);
  root.setProperty("--theme-accent-soft", `color-mix(in srgb, ${colors.accent} 12%, ${colors.background_element})`);
}

export const semanticEventColors = {
  file_edit: "var(--color-info)",
  tool_call: "var(--color-accent)",
  terminal: "var(--color-success)",
  permission: "var(--color-warning)",
  question: "var(--color-warning)",
  status: "var(--color-text-muted)",
} as const;

// ── Agent colour helpers ────────────────────────────────────────

/**
 * Theme-derived colour palette for agent badges.
 * Each entry is a CSS variable that adapts when the theme changes.
 * The order is fixed so hash→index mapping is stable.
 */
const AGENT_PALETTE = [
  "var(--color-primary)",
  "var(--color-secondary)",
  "var(--color-accent)",
  "var(--color-info)",
  "var(--color-success)",
  "var(--color-warning)",
  "var(--color-error)",
] as const;

/**
 * Simple djb2-style string hash. Deterministic — same string always
 * returns the same unsigned 32-bit number.
 */
function hashString(str: string): number {
  let h = 5381;
  for (let i = 0; i < str.length; i++) {
    h = ((h << 5) + h + str.charCodeAt(i)) >>> 0;
  }
  return h;
}

/**
 * Resolve the display colour for an agent.
 *
 * 1. If the server provides a custom colour string, use it as-is.
 * 2. Otherwise, hash the agent id to pick a stable index into the
 *    theme colour palette.  Because the palette uses CSS variables,
 *    the resolved colour automatically follows the active theme.
 */
export function agentColor(id: string, custom?: string): string {
  if (custom) return custom;
  const idx = hashString(id.toLowerCase()) % AGENT_PALETTE.length;
  return AGENT_PALETTE[idx];
}

export function withAlpha(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "");
  if (normalized.length !== 6) return `rgba(0, 0, 0, ${alpha})`;
  const r = parseInt(normalized.slice(0, 2), 16);
  const g = parseInt(normalized.slice(2, 4), 16);
  const b = parseInt(normalized.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Determine if a colour is perceptually "dark" using relative luminance.
 * Returns true when luminance < 0.2 (a generous dark threshold).
 */
function isDarkColor(hex: string): boolean {
  const c = hex.replace("#", "");
  if (c.length < 6) return true; // fallback — assume dark
  const r = parseInt(c.slice(0, 2), 16) / 255;
  const g = parseInt(c.slice(2, 4), 16) / 255;
  const b = parseInt(c.slice(4, 6), 16) / 255;
  // sRGB → linear
  const toLinear = (v: number) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const L = 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
  return L < 0.2;
}

/**
 * Update the browser / PWA chrome colour that paints behind the status-bar
 * and the mobile navigation gesture area.  Works for both the standard
 * `<meta name="theme-color">` and the Apple-specific variant.
 *
 * Also syncs `<html>`, `<body>`, `#root` background-color and the
 * `color-scheme` meta so Android Chrome derives the correct navigation-bar
 * tint and iOS respects light/dark safe-area fills.
 */
function syncMetaThemeColor(color: string) {
  // ── 1. theme-color meta tags ─────────────────────────────────
  // Android Chrome uses these for the status-bar colour in standalone
  // mode.  We update ALL theme-color meta tags (including media-scoped
  // ones for prefers-color-scheme) so Android dark-mode overrides
  // cannot force a different colour.
  const allThemeMetas = document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]');
  if (allThemeMetas.length > 0) {
    allThemeMetas.forEach((m) => m.setAttribute("content", color));
  } else {
    // Create all three variants for maximum compatibility
    for (const media of [
      "(prefers-color-scheme: dark)",
      "(prefers-color-scheme: light)",
      "",
    ]) {
      const m = document.createElement("meta");
      m.name = "theme-color";
      m.content = color;
      if (media) m.setAttribute("media", media);
      document.head.appendChild(m);
    }
  }

  // ── 2. color-scheme meta + CSS property ──────────────────────
  // Android Chrome derives the navigation bar dark/light mode from
  // this.  iOS Safari uses it for status-bar text colour.
  const scheme = isDarkColor(color) ? "dark" : "light";
  let schemeMeta = document.querySelector<HTMLMetaElement>('meta[name="color-scheme"]');
  if (schemeMeta) {
    schemeMeta.setAttribute("content", scheme);
  } else {
    schemeMeta = document.createElement("meta");
    schemeMeta.name = "color-scheme";
    schemeMeta.content = scheme;
    document.head.appendChild(schemeMeta);
  }
  document.documentElement.style.setProperty("color-scheme", scheme);

  // ── 3. Paint every layer with the theme background ───────────
  // Chrome on Android samples the page background to derive the
  // bottom gesture/nav bar tint.  We set it on html, body, AND
  // #root so no transparent gap lets the system default bleed in.
  //
  // Using both `background` shorthand and `backgroundColor` to
  // ensure any prior shorthand (from <style> or CSS) is overridden.
  const docEl = document.documentElement;
  docEl.style.background = color;
  docEl.style.backgroundColor = color;
  document.body.style.background = color;
  document.body.style.backgroundColor = color;

  const root = document.getElementById("root");
  if (root) {
    root.style.background = color;
    root.style.backgroundColor = color;
  }
}

// ── Dynamic favicon ─────────────────────────────────────────────────

/**
 * Generate an SVG favicon that matches the current theme and swap it
 * into the existing `<link rel="icon">` element.  The icon is a
 * terminal-style chevron + underscore (matching the static favicon.svg
 * design) but using the theme's primary colour for strokes and the
 * theme's background colour for the fill.
 */
export function updateFavicon(primaryColor: string, bgColor: string) {
  const svg = buildThemeSvg(primaryColor, bgColor);

  const blob = new Blob([svg], { type: "image/svg+xml" });
  const url = URL.createObjectURL(blob);

  // Update existing <link rel="icon"> or create one
  let link = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (link) {
    // Revoke previous blob URL if we set one
    if (link.href.startsWith("blob:")) URL.revokeObjectURL(link.href);
    link.href = url;
  } else {
    link = document.createElement("link");
    link.rel = "icon";
    link.type = "image/svg+xml";
    link.href = url;
    document.head.appendChild(link);
  }
}

// ── Dynamic PWA / apple-touch icons ─────────────────────────────────

/** The canonical SVG template for all icon sizes. */
function buildThemeSvg(primaryColor: string, bgColor: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <rect width="32" height="32" rx="7" fill="${bgColor}"/>
  <path d="M7 22 L14 16 L7 10" stroke="${primaryColor}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
  <line x1="16" y1="22" x2="25" y2="22" stroke="${primaryColor}" stroke-width="2.5" stroke-linecap="round"/>
</svg>`;
}

/**
 * Render the themed SVG into a PNG blob URL at the given pixel size.
 * Uses OffscreenCanvas when available (most modern browsers), falling
 * back to a regular <canvas> element.
 */
function svgToPngBlobUrl(svg: string, size: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const blob = new Blob([svg], { type: "image/svg+xml" });
    const svgUrl = URL.createObjectURL(blob);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(svgUrl);
      try {
        let pngUrl: string;
        if (typeof OffscreenCanvas !== "undefined") {
          const oc = new OffscreenCanvas(size, size);
          const ctx = oc.getContext("2d")!;
          ctx.drawImage(img, 0, 0, size, size);
          // convertToBlob is async
          oc.convertToBlob({ type: "image/png" }).then((pngBlob) => {
            resolve(URL.createObjectURL(pngBlob));
          }).catch(reject);
          return;
        }
        // Fallback: regular canvas
        const canvas = document.createElement("canvas");
        canvas.width = size;
        canvas.height = size;
        const ctx = canvas.getContext("2d")!;
        ctx.drawImage(img, 0, 0, size, size);
        canvas.toBlob((pngBlob) => {
          if (pngBlob) {
            resolve(URL.createObjectURL(pngBlob));
          } else {
            reject(new Error("canvas.toBlob returned null"));
          }
        }, "image/png");
      } catch (e) {
        reject(e);
      }
    };
    img.onerror = () => {
      URL.revokeObjectURL(svgUrl);
      reject(new Error("Failed to load SVG for icon rasterisation"));
    };
    img.src = svgUrl;
  });
}

/** Track previous blob URLs so we can revoke them. */
let _prevAppleTouchUrl: string | null = null;
let _prevManifestUrl: string | null = null;
let _prevIcon192Url: string | null = null;
let _prevIcon512Url: string | null = null;

/**
 * Generate themed PNG icons and update the apple-touch-icon link and
 * the web app manifest so PWA install / home-screen icons follow the
 * active theme.
 *
 * This is intentionally fire-and-forget from applyThemeToCss().
 */
async function updatePwaIcons(primaryColor: string, bgColor: string) {
  const svg = buildThemeSvg(primaryColor, bgColor);

  try {
    // Generate all three sizes in parallel
    const [url192, url512] = await Promise.all([
      svgToPngBlobUrl(svg, 192),
      svgToPngBlobUrl(svg, 512),
    ]);

    // ── apple-touch-icon ─────────────────────────────────────────
    let appleLink = document.querySelector<HTMLLinkElement>('link[rel="apple-touch-icon"]');
    if (appleLink) {
      if (_prevAppleTouchUrl) URL.revokeObjectURL(_prevAppleTouchUrl);
      appleLink.href = url192;
    } else {
      appleLink = document.createElement("link");
      appleLink.rel = "apple-touch-icon";
      appleLink.href = url192;
      document.head.appendChild(appleLink);
    }
    _prevAppleTouchUrl = url192;

    // ── Dynamic manifest with themed icons ───────────────────────
    // We generate a blob manifest.json that references the themed
    // PNG blob URLs.  This means "Add to Home Screen" will pick up
    // the themed icons.
    try {
      // Fetch current manifest to preserve all other fields
      const existingLink = document.querySelector<HTMLLinkElement>('link[rel="manifest"]');
      const manifestUrl = existingLink?.href || "/manifest.json";
      const res = await fetch(manifestUrl);
      if (res.ok) {
        const manifest = await res.json();

        // Replace icon entries with our themed blob URLs
        manifest.icons = [
          { src: url192, sizes: "192x192", type: "image/png", purpose: "any" },
          { src: url512, sizes: "512x512", type: "image/png", purpose: "any" },
          { src: url192, sizes: "192x192", type: "image/png", purpose: "maskable" },
          { src: url512, sizes: "512x512", type: "image/png", purpose: "maskable" },
        ];
        // Also update theme_color and background_color
        manifest.theme_color = bgColor;
        manifest.background_color = bgColor;

        const manifestBlob = new Blob([JSON.stringify(manifest)], {
          type: "application/manifest+json",
        });
        const newManifestUrl = URL.createObjectURL(manifestBlob);

        if (existingLink) {
          if (_prevManifestUrl) URL.revokeObjectURL(_prevManifestUrl);
          existingLink.href = newManifestUrl;
        } else {
          const link = document.createElement("link");
          link.rel = "manifest";
          link.href = newManifestUrl;
          document.head.appendChild(link);
        }

        // Revoke old icon blob URLs
        if (_prevIcon192Url) URL.revokeObjectURL(_prevIcon192Url);
        if (_prevIcon512Url) URL.revokeObjectURL(_prevIcon512Url);
        _prevManifestUrl = newManifestUrl;
        _prevIcon192Url = url192;
        _prevIcon512Url = url512;
      }
    } catch {
      // Manifest update is best-effort; favicon + apple-touch-icon
      // are already updated above.
    }
  } catch {
    // Icon rasterisation not supported — static icons remain as fallback.
  }
}

// ── Service worker theme sync ───────────────────────────────────────

/**
 * Post the current theme's primary + background colours to the active
 * service worker so it can intercept `/manifest.json`, `/favicon.svg`,
 * `/icon-192.png` and `/icon-512.png` requests and return themed
 * versions.  This is what makes an *already-installed* PWA pick up
 * the new icon/splash colours.
 */
function notifyServiceWorker(primaryColor: string, bgColor: string) {
  if (!("serviceWorker" in navigator)) return;
  navigator.serviceWorker.ready.then((reg) => {
    reg.active?.postMessage({
      type: "THEME_COLORS",
      colors: { primary: primaryColor, background: bgColor },
    });
  }).catch(() => { /* SW not available — ignore */ });
}
