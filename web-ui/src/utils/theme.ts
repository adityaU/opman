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

export function withAlpha(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "");
  if (normalized.length !== 6) return `rgba(0, 0, 0, ${alpha})`;
  const r = parseInt(normalized.slice(0, 2), 16);
  const g = parseInt(normalized.slice(2, 4), 16);
  const b = parseInt(normalized.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Update the browser / PWA chrome colour that paints behind the status-bar
 * and the mobile navigation gesture area.  Works for both the standard
 * `<meta name="theme-color">` and the Apple-specific variant.
 *
 * Also syncs the `html` element background so safe-area zones (top status bar,
 * bottom gesture pill) are filled with the correct colour on iOS standalone
 * and Android PWA.
 */
function syncMetaThemeColor(color: string) {
  // 1. Standard theme-color meta (Android Chrome top status bar + tab colour)
  let meta = document.querySelector<HTMLMetaElement>(
    'meta[name="theme-color"]'
  );
  if (!meta) {
    meta = document.createElement("meta");
    meta.name = "theme-color";
    document.head.appendChild(meta);
  }
  meta.content = color;

  // 2. Keep <html> background in sync for safe-area zones (iOS standalone,
  //    Android PWA, and any viewport-fit=cover edges).
  document.documentElement.style.setProperty("background", color);
}
