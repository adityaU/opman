import type { MessagePart } from "../types";
import { ParsedOutput, FILE_CONTENT_RE, TASK_RESULT_RE, EXT_TO_LANG } from "./types";

/**
 * Extract the child session ID for a task tool part.
 *
 * The upstream opencode task tool sets the spawned child session ID via
 * `ctx.metadata({ sessionId })`, which lands at `state.metadata.sessionId`.
 * This is the authoritative source.
 *
 * `state.input.task_id` is only present when the model passes an existing
 * session ID to resume a prior task — used as a secondary source.
 *
 * Falls back to the positionally-matched `childSession.id` from appState.
 */
export function getTaskSessionId(
  part: MessagePart,
  childSession?: { id: string; title: string } | null,
): string | null {
  // Primary: read from state.metadata.sessionId (set by upstream opencode task tool)
  const metaSessionId = part.state?.metadata?.sessionId;
  if (typeof metaSessionId === "string" && metaSessionId.length > 0) {
    return metaSessionId;
  }
  // Secondary: state.input.task_id (resume case — model passes prior session ID)
  const input = part.state?.input;
  if (input && typeof input === "object" && !Array.isArray(input)) {
    const taskId = (input as Record<string, unknown>).task_id;
    if (typeof taskId === "string" && taskId.length > 0) {
      return taskId;
    }
  }
  // Tertiary: positionally-matched session from appState
  return childSession?.id ?? null;
}

/** Parse structured output from tool results */
export function parseOutput(output: string): ParsedOutput {
  // Check for file content XML pattern
  const fileMatch = FILE_CONTENT_RE.exec(output);
  if (fileMatch) {
    return {
      type: "file",
      path: fileMatch[1],
      content: fileMatch[2],
    };
  }

  // Check for task_result markdown pattern
  const taskMatch = TASK_RESULT_RE.exec(output);
  if (taskMatch) {
    return {
      type: "markdown",
      path: "",
      content: taskMatch[1].trim(),
    };
  }

  return { type: "plain", path: "", content: output };
}

/** Guess syntax highlighter language from file path */
export function guessLanguage(path: string): string {
  const filename = path.split("/").pop() || "";
  const lower = filename.toLowerCase();

  // Special filenames
  if (lower === "dockerfile") return "dockerfile";
  if (lower === "makefile" || lower === "gnumakefile") return "makefile";
  if (lower.endsWith(".lock")) return "json";

  const ext = lower.split(".").pop() || "";
  return EXT_TO_LANG[ext] || "text";
}

/** Shorten tool names by removing common prefixes (provider_provider_action -> action) */
export function formatToolName(name: string): string {
  const parts = name.split("_");
  if (parts.length >= 3 && parts[0] === parts[1]) {
    return parts.slice(2).join("_");
  }
  if (parts.length === 2 && parts[0] === parts[1]) {
    return parts[0];
  }
  return name;
}

/** Format milliseconds as a human-readable duration */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return `${m}m ${rem.toFixed(0)}s`;
}
