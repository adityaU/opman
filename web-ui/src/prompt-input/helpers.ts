import type { AgentInfo, ImageAttachment } from "../api";

// ── Agent colour helpers ────────────────────────────────────────

// Re-export the shared theme-derived agentColor utility
export { agentColor } from "../utils/theme";

/** Fallback agents if fetch fails or is pending */
export const DEFAULT_AGENTS: AgentInfo[] = [
  { id: "build", label: "Build", description: "Default coding agent", mode: "primary", native: true },
  { id: "plan", label: "Plan", description: "Planning and design agent", mode: "all", native: true },
];

/**
 * Filter agents the same way opencode does: hide agents with mode "subagent"
 * and those explicitly marked hidden.
 */
export function selectableAgents(agents: AgentInfo[]): AgentInfo[] {
  return agents.filter((a) => a.mode !== "subagent" && !a.hidden);
}

// ── Image attachment helpers ────────────────────────────────────

/** Max file size for image attachments (10 MB) */
export const MAX_IMAGE_SIZE = 10 * 1024 * 1024;

/** Accepted image MIME types */
export const ACCEPTED_IMAGE_TYPES = new Set([
  "image/png", "image/jpeg", "image/gif",
  "image/webp", "image/svg+xml", "image/bmp",
]);

/** Convert a File to an ImageAttachment via base64 */
export function fileToImageAttachment(file: File): Promise<ImageAttachment> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      const base64 = dataUrl.split(",")[1] || "";
      resolve({ base64, mimeType: file.type, name: file.name || "pasted-image" });
    };
    reader.onerror = () => reject(new Error("Failed to read file"));
    reader.readAsDataURL(file);
  });
}

/** Shorten model ID for display */
export function shortModelName(modelId: string): string {
  const parts = modelId.split("/");
  const name = parts[parts.length - 1];
  return name.length > 30 ? name.slice(0, 28) + "\u2026" : name;
}

/** Commands that execute immediately without needing args */
export const NO_ARG_COMMANDS = new Set([
  "new", "cancel", "compact", "undo", "redo", "share", "fork", "terminal", "clear", "models",
  "keys", "keybindings", "todos", "sessions", "context", "settings",
  "gquota", "quota", "quota_status",
  "tokens_today", "tokens_daily", "tokens_weekly", "tokens_monthly", "tokens_all", "tokens_session",
  "system", "htop", "monitor",
]);
