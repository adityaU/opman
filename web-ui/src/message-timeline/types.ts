import type { Message } from "../types";
import type { AppState } from "../api";
import { Code, Bug, Lightbulb, MessageSquare } from "lucide-react";

// ── Types ──────────────────────────────────────────────

export interface MessageTimelineProps {
  messages: Message[];
  sessionStatus: "idle" | "busy";
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

/**
 * Group consecutive same-role messages together.
 *
 * Accepts `prevGroups` so that groups whose identity hasn't changed can keep
 * the same object reference, allowing React.memo on MessageTurn to skip
 * re-rendering unchanged groups during streaming.
 */
export function groupMessages(
  messages: Message[],
  prevGroups?: MessageGroup[],
): MessageGroup[] {
  const groups: MessageGroup[] = [];
  for (const msg of messages) {
    const last = groups[groups.length - 1];
    if (last && last.role === msg.info.role) {
      last.messages.push(msg);
    } else {
      groups.push({
        role: msg.info.role,
        messages: [msg],
        key: msg.info.messageID || `grp-${groups.length}`,
      });
    }
  }

  // If we have previous groups, reuse unchanged group objects so shallow
  // comparison in React.memo will skip re-renders for groups that didn't change.
  if (prevGroups && prevGroups.length > 0) {
    for (let i = 0; i < groups.length && i < prevGroups.length; i++) {
      const prev = prevGroups[i];
      const curr = groups[i];
      if (prev.key !== curr.key) break;
      if (prev.messages.length !== curr.messages.length) continue;
      // Check if the last message in the group is the same reference
      // (earlier messages in a finished group won't change)
      const pLast = prev.messages[prev.messages.length - 1];
      const cLast = curr.messages[curr.messages.length - 1];
      if (pLast === cLast) {
        groups[i] = prev; // reuse previous object reference
      }
    }
  }

  return groups;
}
