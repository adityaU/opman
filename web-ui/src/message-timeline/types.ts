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

/** Group consecutive same-role messages together. */
export function groupMessages(messages: Message[]): MessageGroup[] {
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
  return groups;
}
