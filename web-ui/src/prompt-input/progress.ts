import type { Message } from "../types";

const ACTIVE_STATUSES = new Set(["pending", "running", "in_progress"]);
const REASONING_TYPES = new Set(["reasoning", "thinking", "analysis"]);

/** Codex-style reasoning opens each step with a bold header: `**Exploring the repo**`. */
const HEADER = /\*\*([^*\n]+)\*\*/g;

function reasoningHeader(text: string | undefined): string | null {
  if (!text) return null;
  HEADER.lastIndex = 0;
  let last: string | null = null;
  for (let match = HEADER.exec(text); match; match = HEADER.exec(text)) {
    const header = match[1]?.trim();
    if (header) last = header;
  }
  return last;
}

/**
 * Return the newest live progress line for the composer status row.
 *
 * Runners that narrate their work (codex) emit a bold header per step; that reads
 * far better than a raw tool title, so whichever part is newest wins. Both are
 * transient, so an idle session shows nothing rather than a stale last step.
 */
export function activeProgressText(messages: readonly Message[], busy: boolean): string | null {
  if (!busy) return null;

  for (let messageIndex = messages.length - 1; messageIndex >= 0; messageIndex--) {
    const parts = messages[messageIndex]?.parts ?? [];
    for (let partIndex = parts.length - 1; partIndex >= 0; partIndex--) {
      const part = parts[partIndex];
      if (!part) continue;

      if (REASONING_TYPES.has(part.type)) {
        const header = reasoningHeader(part.text);
        if (header) return header;
        continue;
      }

      const state = part.state;
      const title = state?.title?.trim();
      if (title && state?.status && ACTIVE_STATUSES.has(state.status)) return title;
    }
  }
  return null;
}
