import { withAlpha } from "../utils/theme";

/** Extract the filename from a full path */
export function basename(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

/** Extract directory from a full path */
export function dirname(path: string): string {
  const idx = path.lastIndexOf("/");
  return idx > 0 ? path.substring(0, idx) : "";
}

/** True when the app is currently showing its light appearance. */
export function isLightAppearance(): boolean {
  if (typeof document === "undefined") return false;
  return document.documentElement.classList.contains("light-theme");
}

/**
 * Palette for `react-diff-viewer-continued`.
 *
 * The viewer reads exactly one variable set, so `light` has to be filled too —
 * a `dark`-only palette rendered in light mode falls back to library defaults
 * and paints unchanged lines nearly white on a light panel.
 */
export function buildDiffStyles() {
  const css = getComputedStyle(document.documentElement);
  const success = css.getPropertyValue("--color-success").trim() || "#7fd88f";
  const error = css.getPropertyValue("--color-error").trim() || "#e06c75";
  const palette = {
        diffViewerBackground: "var(--theme-surface-1, var(--color-bg))",
        diffViewerColor: "var(--color-text)",
        addedBackground: withAlpha(success, 0.1),
        addedColor: "var(--color-text)",
        removedBackground: withAlpha(error, 0.1),
        removedColor: "var(--color-text)",
        wordAddedBackground: withAlpha(success, 0.22),
        wordRemovedBackground: withAlpha(error, 0.22),
        addedGutterBackground: withAlpha(success, 0.14),
        removedGutterBackground: withAlpha(error, 0.14),
        gutterBackground: "var(--theme-surface-2, var(--color-bg-element))",
        gutterColor: "var(--color-text-muted)",
        gutterBackgroundDark: "var(--theme-surface-1, var(--color-bg))",
        highlightBackground:
          "var(--theme-surface-hover, var(--color-surface-hover))",
        highlightGutterBackground:
          "var(--theme-surface-3, var(--color-bg-element))",
        codeFoldGutterBackground:
          "var(--theme-surface-2, var(--color-bg-element))",
        codeFoldBackground: "var(--theme-surface-2, var(--color-bg-element))",
        emptyLineBackground: "var(--theme-surface-1, var(--color-bg))",
        codeFoldContentColor: "var(--color-text-muted)",
  };
  return {
    variables: { dark: palette, light: palette },
    line: {
      fontFamily: "var(--font-mono)",
      fontSize: "12px",
    },
    gutter: {
      minWidth: "36px",
    },
    contentText: {
      fontFamily: "var(--font-mono)",
      fontSize: "12px",
      lineHeight: "1.5",
    },
  };
}
