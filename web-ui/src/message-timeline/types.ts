import type { Message } from "../types";
import type { AppState } from "../api";
import type { SessionStatus } from "../hooks/sse/types";
import { Code, Bug, Lightbulb, MessageSquare } from "lucide-react";

// ── Types ──────────────────────────────────────────────

export interface MessageTimelineProps {
  messages: Message[];
  sessionStatus: SessionStatus;
  activeSessionId: string | null;
  isLoadingMessages?: boolean;
  isLoadingOlder?: boolean;
  hasOlderMessages?: boolean;
  totalMessageCount?: number;
  onLoadOlder?: () => Promise<boolean>;
  appState?: AppState | null;
  defaultModel?: string | null;
  onSendPrompt?: (text: string) => void;
  subagentMessages?: Map<string, Message[]>;
  searchMatchIds?: Set<string>;
  activeSearchMatchId?: string | null;
  isBookmarked?: (messageId: string) => boolean;
  onToggleBookmark?: (messageId: string, sessionId: string, role: string, preview: string) => void;
  onScrollDirection?: (direction: "up" | "down") => void;
  onOpenSession?: (sessionId: string) => void;
}

/** A group of consecutive messages sharing the same role. */
export interface MessageGroup {
  role: string;
  messages: Message[];
  key: string;
}

// ── Constants ──────────────────────────────────────────

/**
 * Threshold (in groups) below which we skip virtualization.
 * For small conversations, plain rendering is cheaper than the virtualizer overhead.
 */
export const VIRTUALIZE_THRESHOLD = 40;

export const SCROLL_DIRECTION_THRESHOLD = 20;

/** Example prompts shown on the new session empty state */
export const EXAMPLE_PROMPTS = [
  { icon: Code, text: "Refactor the auth module to use JWT tokens" },
  { icon: Bug, text: "Find and fix the memory leak in the worker pool" },
  { icon: Lightbulb, text: "Add unit tests for the API endpoints" },
  { icon: MessageSquare, text: "Explain the architecture of this project" },
];

// ── Helpers ────────────────────────────────────────────

function isToolOnlyMessage(message: Message): boolean {
  return message.parts.length > 0 && message.parts.every((part) =>
    part.type === "tool" || part.type === "tool-call" || part.type === "tool_call" ||
    ["step-start", "step-finish", "snapshot", "patch"].includes(part.type),
  );
}

/**
 * Keep prose and reasoning as separate visual turns. Consecutive tool-only
 * messages may share a turn so the tool renderer can collapse their calls.
 */
export function groupMessages(
  messages: Message[],
  prevGroups?: MessageGroup[],
): MessageGroup[] {
  const groups: MessageGroup[] = [];
  for (const msg of messages) {
    const last = groups[groups.length - 1];
    const canJoinToolTurn = last && last.role === "assistant" &&
      isToolOnlyMessage(msg) && isToolOnlyMessage(last.messages[last.messages.length - 1]);
    if (canJoinToolTurn) {
      last.messages.push(msg);
    } else {
      groups.push({
        role: msg.info.role,
        messages: [msg],
        key: msg.info.messageID || msg.info.id || `grp-${groups.length}`,
      });
    }
  }

  // Reuse unchanged groups so streaming updates still avoid rerendering the
  // completed transcript above the active message.
  if (prevGroups && prevGroups.length > 0) {
    for (let i = 0; i < groups.length && i < prevGroups.length; i++) {
      const prev = prevGroups[i];
      const curr = groups[i];
      if (prev.key !== curr.key) break;
      if (prev.messages.length !== curr.messages.length) continue;
      const previousLast = prev.messages[prev.messages.length - 1];
      const currentLast = curr.messages[curr.messages.length - 1];
      if (previousLast === currentLast) groups[i] = prev;
    }
  }

  return groups;
}
