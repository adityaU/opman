import type { Message, MessagePart, ModelRef } from "../types";
import type { MessageGroup } from "../MessageTimeline";
import type { SessionInfo } from "../api";

export type { Message, MessagePart, ModelRef, MessageGroup, SessionInfo };

export interface MessageTurnProps {
  group: MessageGroup;
  /** Child sessions for the active session, sorted by creation time */
  childSessions?: SessionInfo[];
  /** Callback to re-send a user message (retry) */
  onRetry?: (text: string) => void;
  /** SSE-driven subagent messages, keyed by session ID */
  subagentMessages?: Map<string, Message[]>;
  /** Message IDs that match the current search query */
  searchMatchIds?: Set<string>;
  /** The currently active (focused) search match message ID */
  activeSearchMatchId?: string | null;
  /** Check if a message is bookmarked */
  isBookmarked?: (messageId: string) => boolean;
  /** Toggle bookmark on a message */
  onToggleBookmark?: (messageId: string, sessionId: string, role: string, preview: string) => void;
  /** Current session ID (for bookmarks) */
  sessionId?: string | null;
  /** Callback to navigate to a child session */
  onOpenSession?: (sessionId: string) => void;
  /** ID of the last assistant message that hasn't completed yet (for "Queued" badge on user messages) */
  pendingAssistantId?: string | null;
}

/** File extension mapping for common languages */
export const LANG_EXTENSIONS: Record<string, string> = {
  javascript: "js", typescript: "ts", python: "py", rust: "rs", ruby: "rb",
  go: "go", java: "java", kotlin: "kt", swift: "swift", csharp: "cs",
  cpp: "cpp", c: "c", html: "html", css: "css", json: "json", yaml: "yml",
  toml: "toml", markdown: "md", bash: "sh", shell: "sh", sql: "sql",
  xml: "xml", jsx: "jsx", tsx: "tsx", php: "php", lua: "lua", zig: "zig",
};
