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

export function buildDiffStyles() {
  const css = getComputedStyle(document.documentElement);
  const success = css.getPropertyValue("--color-success").trim() || "#7fd88f";
  const error = css.getPropertyValue("--color-error").trim() || "#e06c75";
  return {
    variables: {
      dark: {
        diffViewerBackground: "var(--theme-surface-1, var(--color-bg))",
        diffViewerColor: "var(--color-text)",
        addedBackground: withAlpha(success, 0.1),
        addedColor: "var(--color-success)",
        removedBackground: withAlpha(error, 0.1),
        removedColor: "var(--color-error)",
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
      },
    },
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
