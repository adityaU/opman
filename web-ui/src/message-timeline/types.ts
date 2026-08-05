import type { Message } from "../types";
import type { AppState } from "../api";
import type { SessionStatus } from "../hooks/sse/types";
import { Code, Bug, Lightbulb, MessageSquare } from "lucide-react";

// ── Types ──────────────────────────────────────────────

export interface MessageTimelineProps {
  messages: Message[];
  sessionStatus: SessionStatus;
  activeSessionId: string | null;
  /** A prompt submitted from this client is still in flight. Runners that report
   *  busy state asynchronously leave a gap this covers. */
  isSending?: boolean;
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

/**
 * How long an unanswered prompt is given before the timeline calls it
 * unanswered. A send resolves as soon as the runner is spawned, and the browser
 * only learns the session is busy on the next app-state push, so there is a
 * short window where nothing looks in flight while the turn is in fact running.
 */
export const NO_RESPONSE_GRACE_MS = 8000;

/** Example prompts shown on the new session empty state */
export const EXAMPLE_PROMPTS = [
  { icon: Code, text: "Refactor the auth module to use JWT tokens" },
  { icon: Bug, text: "Find and fix the memory leak in the worker pool" },
  { icon: Lightbulb, text: "Add unit tests for the API endpoints" },
  { icon: MessageSquare, text: "Explain the architecture of this project" },
];

// ── Helpers ────────────────────────────────────────────

/**
 * A runner can stream one assistant turn as several message records. Keep
 * those records together until the conversation actually changes speaker.
 * An explicit agent change is also a boundary so a handoff is not presented
 * as one agent's answer.
 */
function canJoinAssistantTurn(last: MessageGroup, message: Message): boolean {
  if (last.role !== "assistant" || message.info.role !== "assistant") return false;

  const previousAgent = last.messages[last.messages.length - 1]?.info.agent;
  const nextAgent = message.info.agent;
  return !previousAgent || !nextAgent || previousAgent === nextAgent;
}

/**
 * Keep all records from one assistant turn in one visual turn. This matters
 * for runners such as Codex that emit text, tool calls, and more text as
 * separate records while the model is still answering.
 */
export function groupMessages(
  messages: Message[],
  prevGroups?: MessageGroup[],
): MessageGroup[] {
  const groups: MessageGroup[] = [];
  for (const msg of messages) {
    const last = groups[groups.length - 1];
    if (last && canJoinAssistantTurn(last, msg)) {
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
