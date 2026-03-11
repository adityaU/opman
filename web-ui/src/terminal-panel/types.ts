import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";

// ── Types ──────────────────────────────────────────────

export type PtyKind = "shell" | "neovim" | "git" | "opencode";
export type TabStatus = "connecting" | "ready" | "error";

export interface TabInfo {
  id: string;
  kind: PtyKind;
  label: string;
  status: TabStatus;
}

export interface TerminalPanelProps {
  sessionId: string | null;
  onClose: () => void;
  /** Whether the panel is currently visible (used to re-fit xterm on reopen) */
  visible?: boolean;
  /** MCP: whether an AI agent is currently using terminal tools */
  mcpAgentActive?: boolean;
}

export interface TabRuntime {
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
  sse: EventSource | null;
  observer: ResizeObserver | null;
  container: HTMLDivElement | null;
}

// ── Constants ──────────────────────────────────────────

export const KIND_LABELS: Record<PtyKind, string> = {
  shell: "Shell",
  neovim: "Neovim",
  git: "Git",
  opencode: "OpenCode",
};

export const TERM_OPTIONS = {
  cursorBlink: true,
  cursorStyle: "block" as const,
  fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
  fontSize: 13,
  lineHeight: 1.2,
  allowTransparency: true,
  allowProposedApi: true,
};

export const ALL_PTY_KINDS: PtyKind[] = ["shell", "neovim", "git", "opencode"];

// ── Helpers ────────────────────────────────────────────

export function uuid(): string {
  return (
    crypto.randomUUID?.() ??
    `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
  );
}

export function getTerminalTheme() {
  const css = getComputedStyle(document.documentElement);
  const text = css.getPropertyValue("--color-text").trim() || "var(--color-text)";
  const muted = css.getPropertyValue("--color-text-muted").trim() || "var(--color-text-muted)";
  const primary = css.getPropertyValue("--color-primary").trim() || "var(--color-primary)";
  const secondary = css.getPropertyValue("--color-secondary").trim() || "var(--color-secondary)";
  const accent = css.getPropertyValue("--color-accent").trim() || "var(--color-accent)";
  const success = css.getPropertyValue("--color-success").trim() || "var(--color-success)";
  const warning = css.getPropertyValue("--color-warning").trim() || "var(--color-warning)";
  const error = css.getPropertyValue("--color-error").trim() || "var(--color-error)";
  const info = css.getPropertyValue("--color-info").trim() || "var(--color-info)";
  const panel = css.getPropertyValue("--color-bg-panel").trim() || "var(--color-bg-panel)";
  return {
    background: "transparent",
    foreground: text,
    cursor: text,
    selectionBackground: `color-mix(in srgb, ${secondary} 28%, transparent)`,
    black: panel,
    red: error,
    green: success,
    yellow: warning,
    blue: secondary,
    magenta: accent,
    cyan: info,
    white: text,
    brightBlack: muted,
    brightRed: error,
    brightGreen: success,
    brightYellow: warning,
    brightBlue: secondary,
    brightMagenta: accent,
    brightCyan: info,
    brightWhite: primary,
    selectionForeground: text,
  };
}
