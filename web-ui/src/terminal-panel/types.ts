import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";

// ── Types ──────────────────────────────────────────────

export type PtyKind = "shell" | "neovim" | "git" | "opencode" | "claude-attach";
export type TabStatus = "connecting" | "ready" | "error";

export interface TabInfo {
  id: string;
  kind: PtyKind;
  label: string;
  status: TabStatus;
  projectKey: string;
}

export interface TerminalPanelProps {
  sessionId: string | null;
  /** Path of the active project — used to scope terminal tabs per project so
   *  switching projects doesn't tear down other projects' running terminals. */
  projectPath: string | null;
  onClose: () => void;
  /** Whether the panel is currently visible (used to re-fit xterm on reopen) */
  visible?: boolean;
  /** MCP: whether an AI agent is currently using terminal tools */
  mcpAgentActive?: boolean;
  /** Bumped each time the user requests a fresh attach tab (e.g. the input's
   *  "Attach terminal" button). On change, a new tab of `attachKind` is created. */
  attachNonce?: number;
  /** Kind to create when `attachNonce` changes (defaults to "claude-attach"). */
  attachKind?: PtyKind;
  /** Which shell renders the panel. Mobile adds the on-screen key bar, because
   *  a soft keyboard has no Esc, Tab, Ctrl or arrows. */
  layout?: "desktop" | "mobile";
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
  "claude-attach": "Claude",
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

/**
 * Build the xterm palette from whichever theme is currently applied.
 *
 * Every colour is resolved to a literal here. xterm parses these itself and
 * understands neither `var()` nor `color-mix()`, so anything left symbolic is
 * silently dropped and the terminal falls back to its own defaults — which is
 * how a themed app ends up with a terminal that belongs to no theme at all.
 */
export function getTerminalTheme() {
  const css = getComputedStyle(document.documentElement);
  const read = (name: string, fallback: string) => {
    const value = css.getPropertyValue(name).trim();
    // A computed value can still be symbolic if the variable is unset or
    // defined in terms of another it cannot resolve.
    if (!value || value.includes("var(") || value.includes("color-mix(")) return fallback;
    return value;
  };

  const light = document.documentElement.classList.contains("light-theme");
  const text = read("--color-text", light ? "#1c1c1f" : "#e6e6e6");
  const muted = read("--color-text-muted", light ? "#6b6b76" : "#8a8a94");
  const primary = read("--color-primary", "#7c5cff");
  const secondary = read("--color-secondary", "#4a9eff");
  const accent = read("--color-accent", "#c76cff");
  const success = read("--color-success", "#3fb950");
  const warning = read("--color-warning", "#d29922");
  const error = read("--color-error", "#f85149");
  const info = read("--color-info", "#58a6ff");
  const surface = read("--color-bg-panel", light ? "#ffffff" : "#16161a");

  // ANSI black and white are ends of an ink scale, not theme roles. On a light
  // terminal "black" has to be the darkest ink and "white" the paper, or every
  // program that prints in black writes in invisible ink.
  const ink = light ? text : surface;
  const paper = light ? surface : text;

  return {
    background: surface,
    foreground: text,
    cursor: primary,
    cursorAccent: surface,
    selectionBackground: withAlpha(primary, light ? 0.22 : 0.32),
    selectionForeground: text,
    black: ink,
    red: error,
    green: success,
    yellow: warning,
    blue: secondary,
    magenta: accent,
    cyan: info,
    white: paper,
    // Bright is a real second rank, not a duplicate of the base colour: without
    // separation, output that leans on bright/dim for structure goes flat.
    brightBlack: muted,
    brightRed: shift(error, light),
    brightGreen: shift(success, light),
    brightYellow: shift(warning, light),
    brightBlue: shift(secondary, light),
    brightMagenta: shift(accent, light),
    brightCyan: shift(info, light),
    brightWhite: light ? text : paper,
  };
}

/** Parse `#rgb`, `#rrggbb`, or `rgb()/rgba()` into channels. */
function channels(color: string): [number, number, number] | null {
  const hex = color.trim();
  if (hex.startsWith("#")) {
    const body = hex.slice(1);
    if (body.length === 3) {
      return [0, 1, 2].map((i) => parseInt(body[i] + body[i], 16)) as [number, number, number];
    }
    if (body.length >= 6) {
      return [0, 2, 4].map((i) => parseInt(body.slice(i, i + 2), 16)) as [number, number, number];
    }
    return null;
  }
  const parts = hex.match(/-?\d+(\.\d+)?/g);
  if (!parts || parts.length < 3) return null;
  return [Number(parts[0]), Number(parts[1]), Number(parts[2])];
}

function withAlpha(color: string, alpha: number): string {
  const rgb = channels(color);
  if (!rgb) return `rgba(124, 92, 255, ${alpha})`;
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})`;
}

/** Push a colour one rank away from the surface it sits on. */
function shift(color: string, light: boolean): string {
  const rgb = channels(color);
  if (!rgb) return color;
  const amount = light ? -38 : 34;
  const moved = rgb.map((c) => Math.max(0, Math.min(255, Math.round(c + amount))));
  return `#${moved.map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}
