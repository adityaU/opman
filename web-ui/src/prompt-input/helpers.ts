import type { AgentInfo, ImageAttachment } from "../api";
import type { SlashCommand } from "../types";

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

/** Display names for the engines a session can run on. */
export const RUNNER_LABELS: Record<string, string> = {
  opencode: "OpenCode",
  "claude-code": "Claude Code",
  claude: "Claude",
  codex: "Codex",
};

/**
 * Whether picking a command should leave the composer open for arguments.
 *
 * Asked of the command itself rather than of a list of names: an ACP agent states its
 * argument hint, an opencode command's template says whether it interpolates any, and
 * opman's own commands open a surface and take none. A name opman has never seen is
 * therefore classified correctly by whatever its runner said about it.
 */
export function takesArguments(command: SlashCommand): boolean {
  if (command.args) return true;
  return /\$ARGUMENTS|\$\d/.test(command.template ?? "");
}
