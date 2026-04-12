/**
 * Pure utility functions for the git panel — no React dependencies.
 */
import type { ThemeColors, GitView, GitTab } from "./types";

// ── Status helpers ──────────────────────────────────────

export function statusLabel(status: string): string {
  switch (status) {
    case "M": return "Modified";
    case "A": return "Added";
    case "D": return "Deleted";
    case "R": return "Renamed";
    case "?": return "Untracked";
    case "U": return "Unmerged";
    default:  return status;
  }
}

export function statusColor(status: string): string {
  switch (status) {
    case "M": return "var(--color-warning)";
    case "A": return "var(--color-success)";
    case "D": return "var(--color-error)";
    case "?": return "var(--color-text-muted)";
    default:  return "var(--color-text)";
  }
}

// ── Color conversion ────────────────────────────────────

export function hexToRgba(hex: string, alpha: number): string {
  const h = hex.replace("#", "");
  const r = parseInt(h.substring(0, 2), 16) || 0;
  const g = parseInt(h.substring(2, 4), 16) || 0;
  const b = parseInt(h.substring(4, 6), 16) || 0;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// ── Diff theme builder ──────────────────────────────────

export function buildDiffStyles(theme: ThemeColors | null) {
  const css = typeof window !== "undefined"
    ? getComputedStyle(document.documentElement)
    : null;
  const success   = theme?.success    || css?.getPropertyValue("--color-success").trim()    || "#7fd88f";
  const error     = theme?.error      || css?.getPropertyValue("--color-error").trim()      || "#e06c75";
  const textMuted = theme?.text_muted || css?.getPropertyValue("--color-text-muted").trim() || "#808080";

  return {
    variables: {
      dark: {
        diffViewerBackground:    "transparent",
        gutterBackground:        "transparent",
        addedBackground:         hexToRgba(success, 0.12),
        addedColor:              success,
        removedBackground:       hexToRgba(error, 0.12),
        removedColor:            error,
        wordAddedBackground:     hexToRgba(success, 0.25),
        wordRemovedBackground:   hexToRgba(error, 0.25),
        addedGutterBackground:   hexToRgba(success, 0.08),
        removedGutterBackground: hexToRgba(error, 0.08),
        gutterColor:             textMuted,
        codeFoldGutterBackground: "transparent",
        codeFoldBackground:      "var(--theme-surface-3, var(--color-bg-element))",
        emptyLineBackground:     "transparent",
        codeFoldContentColor:    textMuted,
      },
    },
    contentText: {
      fontFamily: "var(--font-mono, monospace)",
      fontSize:   "12px",
      lineHeight: "1.5",
    },
  };
}

// ── Diff parsing ────────────────────────────────────────

/**
 * Split a unified diff containing multiple files into a Map keyed by file path.
 */
export function splitDiffByFile(fullDiff: string): Map<string, string> {
  const result = new Map<string, string>();
  if (!fullDiff.trim()) return result;

  const parts = fullDiff.split(/(?=^diff --git )/m);
  for (const part of parts) {
    const trimmed = part.trim();
    if (!trimmed.startsWith("diff --git")) continue;
    const headerMatch = trimmed.match(/^diff --git a\/(.+?) b\/(.+)/m);
    if (headerMatch) {
      result.set(headerMatch[2], trimmed);
    }
  }
  return result;
}

/**
 * Parse a unified diff into old (removed) and new (added) text for the diff viewer.
 */
export function parseUnifiedDiff(diff: string): { oldText: string; newText: string } {
  if (!diff.trim()) return { oldText: "", newText: "" };

  const oldLines: string[] = [];
  const newLines: string[] = [];
  let inHunk = false;

  for (const line of diff.split("\n")) {
    if (line.startsWith("@@")) { inHunk = true; continue; }
    if (!inHunk) continue;

    if (line.startsWith("-")) {
      oldLines.push(line.slice(1));
    } else if (line.startsWith("+")) {
      newLines.push(line.slice(1));
    } else if (line.startsWith(" ")) {
      oldLines.push(line.slice(1));
      newLines.push(line.slice(1));
    } else if (line === "\\ No newline at end of file") {
      // skip
    } else {
      oldLines.push(line);
      newLines.push(line);
    }
  }

  return { oldText: oldLines.join("\n"), newText: newLines.join("\n") };
}

// ── Time formatting ─────────────────────────────────────

export function formatRelativeTime(isoDate: string): string {
  try {
    const diffMs   = Date.now() - new Date(isoDate).getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1)  return "just now";
    if (diffMins < 60) return `${diffMins}m ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 30) return `${diffDays}d ago`;
    const diffMonths = Math.floor(diffDays / 30);
    if (diffMonths < 12) return `${diffMonths}mo ago`;
    return `${Math.floor(diffMonths / 12)}y ago`;
  } catch {
    return isoDate;
  }
}

// ── Breadcrumb label ────────────────────────────────────

export function breadcrumbLabel(view: GitView, tab: GitTab): string {
  switch (view.kind) {
    case "list":      return tab === "changes" ? "Changes" : "Log";
    case "file-diff": return view.file.split("/").pop() || view.file;
    case "commit":    return view.shortHash;
  }
}
