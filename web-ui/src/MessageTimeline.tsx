import React, { useRef, useEffect, useCallback, useMemo, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, FolderOpen, Cpu, MessageSquare, Code, Bug, Lightbulb } from "lucide-react";
import type { Message } from "./types";
import type { AppState, SessionInfo } from "./api";
import { MessageTurn } from "./MessageTurn";

interface Props {
  messages: Message[];
  sessionStatus: "idle" | "busy";
  activeSessionId: string | null;
  /** True while messages are being fetched for a newly-selected session */
  isLoadingMessages?: boolean;
  /** True while older messages are being fetched (pagination) */
  isLoadingOlder?: boolean;
  /** True if there are older messages available to load */
  hasOlderMessages?: boolean;
  /** Total message count in the session (for display) */
  totalMessageCount?: number;
  /** Callback to load older messages (pagination). Returns true if more exist. */
  onLoadOlder?: () => Promise<boolean>;
  /** App state containing all projects and sessions (optional for subagent views) */
  appState?: AppState | null;
  /** Default model display string (e.g. "claude-sonnet-4-20250514") */
  defaultModel?: string | null;
  /** Callback to send example prompt text */
  onSendPrompt?: (text: string) => void;
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
  /** Reports scroll direction when cumulative delta exceeds threshold (mobile input autohide) */
  onScrollDirection?: (direction: "up" | "down") => void;
  /** Callback to navigate to a child/subagent session */
  onOpenSession?: (sessionId: string) => void;
}

/** A group of consecutive messages sharing the same role. */
export interface MessageGroup {
  /** The role shared by all messages in this group. */
  role: string;
  /** The ordered list of messages in this group. */
  messages: Message[];
  /** Stable key derived from the first message's ID. */
  key: string;
}

/**
 * Threshold (in groups) below which we skip virtualization.
 * For small conversations, plain rendering is cheaper than the virtualizer overhead.
 */
const VIRTUALIZE_THRESHOLD = 40;

/** Group consecutive same-role messages together. */
function groupMessages(messages: Message[]): MessageGroup[] {
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

/** Shimmer skeleton shown while messages are loading.
 *  Mirrors the real `.message-turn` layout: avatar (32px) + content column. */
function MessageShimmer() {
  return (
    <div className="message-shimmer" aria-label="Loading messages">
      {/* User message skeleton */}
      <div className="shimmer-turn shimmer-user">
        <div className="shimmer-avatar" />
        <div className="shimmer-content">
          <div className="shimmer-line shimmer-role-label" />
          <div className="shimmer-line shimmer-w-55" />
          <div className="shimmer-line shimmer-w-35" />
        </div>
      </div>
      {/* Assistant message skeleton */}
      <div className="shimmer-turn shimmer-assistant">
        <div className="shimmer-avatar" />
        <div className="shimmer-content">
          <div className="shimmer-line shimmer-role-label" />
          <div className="shimmer-line shimmer-w-90" />
          <div className="shimmer-line shimmer-w-75" />
          <div className="shimmer-line shimmer-w-60" />
          <div className="shimmer-line shimmer-w-45" />
        </div>
      </div>
      {/* Second user message skeleton */}
      <div className="shimmer-turn shimmer-user">
        <div className="shimmer-avatar" />
        <div className="shimmer-content">
          <div className="shimmer-line shimmer-role-label" />
          <div className="shimmer-line shimmer-w-40" />
        </div>
      </div>
      {/* Second assistant message skeleton */}
      <div className="shimmer-turn shimmer-assistant">
        <div className="shimmer-avatar" />
        <div className="shimmer-content">
          <div className="shimmer-line shimmer-role-label" />
          <div className="shimmer-line shimmer-w-80" />
          <div className="shimmer-line shimmer-w-65" />
          <div className="shimmer-line shimmer-w-50" />
        </div>
      </div>
    </div>
  );
}

/** Example prompts shown on the new session empty state */
const EXAMPLE_PROMPTS = [
  { icon: Code, text: "Refactor the auth module to use JWT tokens" },
  { icon: Bug, text: "Find and fix the memory leak in the worker pool" },
  { icon: Lightbulb, text: "Add unit tests for the API endpoints" },
  { icon: MessageSquare, text: "Explain the architecture of this project" },
];

export function MessageTimeline({
  messages,
  sessionStatus,
  activeSessionId,
  isLoadingMessages = false,
  isLoadingOlder = false,
  hasOlderMessages = false,
  totalMessageCount = 0,
  onLoadOlder,
  appState,
  defaultModel,
  onSendPrompt,
  subagentMessages,
  searchMatchIds,
  activeSearchMatchId,
  isBookmarked,
  onToggleBookmark,
  onScrollDirection,
  onOpenSession,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  /** Tracks whether we should auto-scroll to bottom (user is near bottom). */
  const shouldAutoScrollRef = useRef(true);
  /** Show "Jump to bottom" button when user scrolls up */
  const [showJumpToBottom, setShowJumpToBottom] = useState(false);
  /** Guard against duplicate loadOlder calls */
  const loadOlderLockRef = useRef(false);

  // ── Scroll direction detection (for mobile input autohide) ──
  const lastScrollTopRef = useRef(0);
  const cumulativeDeltaRef = useRef(0);
  const directionRafRef = useRef(0);
  const SCROLL_DIRECTION_THRESHOLD = 20;

  // Derive project/session directory for empty state display
  const sessionDirectory = useMemo(() => {
    if (!appState || !activeSessionId) return null;
    const project = appState.projects[appState.active_project];
    if (!project) return null;
    // Try to find the active session's directory first
    const session = project.sessions.find((s) => s.id === activeSessionId);
    if (session?.directory) return session.directory;
    // Fall back to project path
    return project.path || null;
  }, [appState, activeSessionId]);

  // Group consecutive same-role messages
  const groups = useMemo(() => groupMessages(messages), [messages]);

  // Find child sessions for the active session, sorted by creation time.
  // These are passed to MessageTurn so each task tool can be matched
  // to its child session by index (N-th task tool -> N-th child session).
  const childSessions = useMemo(() => {
    if (!appState || !activeSessionId) return [];
    const children: SessionInfo[] = [];
    for (const project of appState.projects) {
      for (const session of project.sessions) {
        if (session.parentID === activeSessionId) {
          children.push(session);
        }
      }
    }
    // Sort by creation time so ordering matches tool call order
    children.sort((a, b) => a.time.created - b.time.created);
    return children;
  }, [appState, activeSessionId]);

  const itemCount = groups.length;
  const useVirtual = groups.length >= VIRTUALIZE_THRESHOLD;

  // ── Virtualizer (only used when group count exceeds threshold) ──

  const virtualizer = useVirtualizer({
    count: itemCount,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 160, // rough estimate; dynamic measurement corrects this
    overscan: 5,
  });

  // ── Auto-scroll ──

  // Scroll listener: track if user is near bottom + detect scroll direction
  // + trigger load-older when near top
  //
  // Key behavior:
  //  - tailing mode (shouldAutoScrollRef=true) is ON by default
  //  - only turns OFF when user explicitly scrolls UP (negative delta)
  //  - turns back ON when user scrolls to bottom or clicks "Jump to bottom"
  //  - content growth (new messages appended) does NOT disable tailing
  const programmaticScrollRef = useRef(false);

  const handleScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;
    const distFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight;
    const nearBottom = distFromBottom < 100;

    // If this scroll was triggered by our own programmatic scroll, skip logic
    if (programmaticScrollRef.current) {
      programmaticScrollRef.current = false;
      setShowJumpToBottom(!nearBottom);
      return;
    }

    const currentScrollTop = container.scrollTop;
    const delta = currentScrollTop - lastScrollTopRef.current;
    lastScrollTopRef.current = currentScrollTop;

    // User scrolled UP → disable tailing
    if (delta < -5) {
      shouldAutoScrollRef.current = false;
    }

    // User reached bottom → re-enable tailing
    if (nearBottom) {
      shouldAutoScrollRef.current = true;
    }

    setShowJumpToBottom(!shouldAutoScrollRef.current && !nearBottom);

    // ── Load older messages when near top ──
    if (
      onLoadOlder &&
      hasOlderMessages &&
      !loadOlderLockRef.current &&
      container.scrollTop < 200
    ) {
      loadOlderLockRef.current = true;
      const prevScrollHeight = container.scrollHeight;
      onLoadOlder().finally(() => {
        // Restore scroll position after older messages are prepended
        requestAnimationFrame(() => {
          const newScrollHeight = container.scrollHeight;
          const heightDiff = newScrollHeight - prevScrollHeight;
          if (heightDiff > 0) {
            container.scrollTop += heightDiff;
          }
          loadOlderLockRef.current = false;
        });
      });
    }

    // ── Scroll direction detection (rAF-throttled) ──
    if (onScrollDirection) {
      // If direction reversed, reset cumulative delta
      if ((cumulativeDeltaRef.current > 0 && delta < 0) ||
          (cumulativeDeltaRef.current < 0 && delta > 0)) {
        cumulativeDeltaRef.current = 0;
      }
      cumulativeDeltaRef.current += delta;

      if (Math.abs(cumulativeDeltaRef.current) >= SCROLL_DIRECTION_THRESHOLD) {
        const direction = cumulativeDeltaRef.current < 0 ? "up" : "down";
        cumulativeDeltaRef.current = 0; // reset after firing
        // Throttle via rAF to avoid excessive calls during momentum scrolling
        if (directionRafRef.current) {
          cancelAnimationFrame(directionRafRef.current);
        }
        directionRafRef.current = requestAnimationFrame(() => {
          onScrollDirection(direction);
          directionRafRef.current = 0;
        });
      }
    }
  }, [onScrollDirection, onLoadOlder, hasOlderMessages]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    // Reset direction-detection state when the underlying container DOM element
    // changes (e.g. loading shimmer → actual message list). Without this,
    // lastScrollTopRef can hold a stale value from the previous container,
    // causing incorrect delta calculations on the first few scroll events.
    lastScrollTopRef.current = container.scrollTop;
    cumulativeDeltaRef.current = 0;
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => container.removeEventListener("scroll", handleScroll);
    // Re-run when the container DOM element changes (e.g. loading → messages transition)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [handleScroll, messages.length]);

  // Derive a lightweight "content fingerprint" so auto-scroll fires when
  // streaming text is appended to the last message (not just new messages).
  const lastMsg = messages[messages.length - 1];
  const contentFingerprint = lastMsg
    ? (lastMsg.info?.parts?.length ?? 0)
    : 0;

  // Auto-scroll to bottom on new messages / content changes (tailing mode)
  useEffect(() => {
    if (!shouldAutoScrollRef.current) return;

    programmaticScrollRef.current = true;
    if (useVirtual) {
      virtualizer.scrollToIndex(itemCount - 1, { align: "end" });
    } else {
      const container = containerRef.current;
      if (container) {
        container.scrollTop = container.scrollHeight;
      }
    }
  }, [messages.length, sessionStatus, useVirtual, itemCount, virtualizer, contentFingerprint]);

  // Jump to bottom handler — re-enables tailing mode
  const scrollToBottom = useCallback(() => {
    programmaticScrollRef.current = true;
    if (useVirtual) {
      virtualizer.scrollToIndex(itemCount - 1, { align: "end" });
    } else {
      const container = containerRef.current;
      if (container) {
        container.scrollTop = container.scrollHeight;
      }
    }
    shouldAutoScrollRef.current = true;
    setShowJumpToBottom(false);
  }, [useVirtual, virtualizer, itemCount]);

  // ── Search: build a map from message ID → group index for scroll targeting ──
  const messageIdToGroupIndex = useMemo(() => {
    const map = new Map<string, number>();
    groups.forEach((group, idx) => {
      for (const msg of group.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (id) map.set(id, idx);
      }
    });
    return map;
  }, [groups]);

  // Also map message IDs to group keys for DOM querying
  const messageIdToGroupKey = useMemo(() => {
    const map = new Map<string, string>();
    for (const group of groups) {
      for (const msg of group.messages) {
        const id = msg.info.messageID || msg.info.id || "";
        if (id) map.set(id, group.key);
      }
    }
    return map;
  }, [groups]);

  // ── Search: scroll to active match when it changes ──
  useEffect(() => {
    if (!activeSearchMatchId) return;
    const container = containerRef.current;
    if (!container) return;

    const groupKey = messageIdToGroupKey.get(activeSearchMatchId);
    if (!groupKey) return;

    if (useVirtual) {
      // In virtual mode, we must scroll to the group index first to ensure
      // the element is rendered, then scrollIntoView after a tick.
      const groupIdx = messageIdToGroupIndex.get(activeSearchMatchId);
      if (groupIdx !== undefined) {
        virtualizer.scrollToIndex(groupIdx, { align: "center" });
        // After virtualizer positions the element, refine with scrollIntoView
        requestAnimationFrame(() => {
          const el = container.querySelector(
            `[data-group-key="${CSS.escape(groupKey)}"]`
          );
          el?.scrollIntoView({ behavior: "smooth", block: "center" });
        });
      }
    } else {
      // Plain mode: direct DOM query
      const el = container.querySelector(
        `[data-group-key="${CSS.escape(groupKey)}"]`
      );
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
    // Disable auto-scroll after a search scroll so it doesn't fight
    shouldAutoScrollRef.current = false;
  }, [activeSearchMatchId, useVirtual, virtualizer, messageIdToGroupIndex, messageIdToGroupKey]);

  // ── Empty states ──

  if (!activeSessionId) {
    return (
      <div className="message-timeline-empty">
        <div className="message-timeline-welcome">
          <h2>Welcome to OpenCode</h2>
          <p>Select a session from the sidebar or create a new one to start chatting.</p>
          <div className="message-timeline-shortcuts">
            <kbd>Cmd+Shift+N</kbd> New Session
            <kbd>Cmd+Shift+P</kbd> Command Palette
            <kbd>Cmd+'</kbd> Model Picker
          </div>
        </div>
      </div>
    );
  }

  // Show shimmer while loading messages for a session
  if (isLoadingMessages && messages.length === 0) {
    return (
      <div className="message-timeline" ref={containerRef}>
        <div className="message-timeline-inner">
          <MessageShimmer />
        </div>
      </div>
    );
  }

  if (messages.length === 0 && sessionStatus === "idle") {
    return (
      <div className="message-timeline-empty">
        <div className="message-timeline-welcome new-session-welcome">
          <h2>New Session</h2>

          {/* Session info: directory + model */}
          <div className="new-session-info">
            {sessionDirectory && (
              <div className="new-session-info-row">
                <FolderOpen size={14} />
                <span className="new-session-directory" title={sessionDirectory}>
                  {sessionDirectory}
                </span>
              </div>
            )}
            {defaultModel && (
              <div className="new-session-info-row">
                <Cpu size={14} />
                <span className="new-session-model-badge">{defaultModel}</span>
              </div>
            )}
          </div>

          <p>Type a message below or try one of these:</p>

          {/* Example prompts */}
          <div className="new-session-prompts">
            {EXAMPLE_PROMPTS.map((prompt, i) => (
              <button
                key={i}
                className="new-session-prompt-card"
                onClick={() => onSendPrompt?.(prompt.text)}
              >
                <prompt.icon size={16} className="new-session-prompt-icon" />
                <span>{prompt.text}</span>
              </button>
            ))}
          </div>

          {/* Keyboard shortcuts */}
          <div className="message-timeline-shortcuts">
            <kbd>Cmd+'</kbd> Model Picker
            <kbd>Cmd+Shift+E</kbd> Editor
            <kbd>Cmd+Shift+G</kbd> Git
          </div>
        </div>
      </div>
    );
  }

  // ── Subtle loading indicator when fetching older messages on scroll ──

  const olderMessagesIndicator = isLoadingOlder ? (
    <div className="load-older-messages">
      <span className="load-older-spinner">Loading older messages...</span>
    </div>
  ) : null;

  // ── Virtualized rendering (large sessions) ──

  if (useVirtual) {
    const virtualItems = virtualizer.getVirtualItems();

    return (
      <div className="message-timeline" ref={containerRef} role="log" aria-live="polite" aria-label="Chat messages">
        {olderMessagesIndicator}
        <div
          className="message-timeline-inner"
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            position: "relative",
          }}
        >
          {virtualItems.map((virtualRow) => {
            const idx = virtualRow.index;
            const group = groups[idx];
            return (
              <div
                key={group.key}
                data-index={idx}
                data-group-key={group.key}
                ref={virtualizer.measureElement}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <MessageTurn group={group} childSessions={childSessions} onRetry={onSendPrompt} subagentMessages={subagentMessages} searchMatchIds={searchMatchIds} activeSearchMatchId={activeSearchMatchId} isBookmarked={isBookmarked} onToggleBookmark={onToggleBookmark} sessionId={activeSessionId} onOpenSession={onOpenSession} />
              </div>
            );
          })}
        </div>
        {showJumpToBottom && (
          <button className="jump-to-bottom" onClick={scrollToBottom}>
            <ArrowDown size={14} />
            <span>Jump to bottom</span>
          </button>
        )}
      </div>
    );
  }

  // ── Plain rendering (small sessions, < VIRTUALIZE_THRESHOLD) ──

  return (
    <div className="message-timeline" ref={containerRef} role="log" aria-live="polite" aria-label="Chat messages">
      <div className="message-timeline-inner">
        {olderMessagesIndicator}
        {groups.map((group) => (
          <div key={group.key} data-group-key={group.key}>
            <MessageTurn group={group} childSessions={childSessions} onRetry={onSendPrompt} subagentMessages={subagentMessages} searchMatchIds={searchMatchIds} activeSearchMatchId={activeSearchMatchId} isBookmarked={isBookmarked} onToggleBookmark={onToggleBookmark} sessionId={activeSessionId} onOpenSession={onOpenSession} />
          </div>
        ))}
      </div>
      {showJumpToBottom && (
        <button className="jump-to-bottom" onClick={scrollToBottom}>
          <ArrowDown size={14} />
          <span>Jump to bottom</span>
        </button>
      )}
    </div>
  );
}
