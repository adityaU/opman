import React, { useRef, useEffect, useCallback } from "react";
import type { Message } from "./types";
import { MessageTurn } from "./MessageTurn";
import { Loader2 } from "lucide-react";

interface Props {
  messages: Message[];
  sessionStatus: "idle" | "busy";
  activeSessionId: string | null;
  /** Called when user scrolls to the top and more messages are available. */
  onLoadMore?: () => void;
  /** Whether there are older messages to load. */
  hasMoreMessages?: boolean;
  /** Whether a "load more" request is currently in flight. */
  isLoadingMore?: boolean;
}

/** Threshold in pixels from the top to trigger loading more messages. */
const SCROLL_TOP_THRESHOLD = 80;

export function MessageTimeline({
  messages,
  sessionStatus,
  activeSessionId,
  onLoadMore,
  hasMoreMessages,
  isLoadingMore,
}: Props) {
  const endRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  /** Tracks whether we should auto-scroll to bottom (user is near bottom). */
  const shouldAutoScrollRef = useRef(true);
  /** Used to preserve scroll position when prepending older messages. */
  const prevScrollHeightRef = useRef(0);
  const wasLoadingMoreRef = useRef(false);

  // Detect when "load more" finishes and restore scroll position
  useEffect(() => {
    if (wasLoadingMoreRef.current && !isLoadingMore && containerRef.current) {
      // Older messages were prepended — adjust scrollTop so the same content
      // stays in view (the container's scrollHeight grew by the prepended amount).
      const newScrollHeight = containerRef.current.scrollHeight;
      const delta = newScrollHeight - prevScrollHeightRef.current;
      containerRef.current.scrollTop += delta;
    }
    wasLoadingMoreRef.current = !!isLoadingMore;
  }, [isLoadingMore]);

  // Auto-scroll to bottom on new messages (only if user was already near bottom)
  useEffect(() => {
    if (shouldAutoScrollRef.current) {
      endRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages.length, sessionStatus]);

  // Scroll listener: detect scroll-to-top (load more) and track if user is near bottom
  const handleScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    // Track whether user is near the bottom (for auto-scroll decision)
    const distFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight;
    shouldAutoScrollRef.current = distFromBottom < 100;

    // Trigger "load more" when scrolled near the top
    if (
      container.scrollTop < SCROLL_TOP_THRESHOLD &&
      hasMoreMessages &&
      !isLoadingMore &&
      onLoadMore
    ) {
      // Save current scroll height so we can restore position after prepend
      prevScrollHeightRef.current = container.scrollHeight;
      onLoadMore();
    }
  }, [hasMoreMessages, isLoadingMore, onLoadMore]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => container.removeEventListener("scroll", handleScroll);
  }, [handleScroll]);

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

  if (messages.length === 0 && sessionStatus === "idle") {
    return (
      <div className="message-timeline-empty">
        <div className="message-timeline-welcome">
          <h2>New Session</h2>
          <p>Type a message below to start the conversation.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="message-timeline" ref={containerRef}>
      <div className="message-timeline-inner">
        {/* Loading indicator at top when fetching older messages */}
        {isLoadingMore && (
          <div className="message-loading-more">
            <Loader2 size={14} className="spin" />
            <span>Loading older messages...</span>
          </div>
        )}
        {hasMoreMessages && !isLoadingMore && (
          <div className="message-load-more-hint">
            <span>Scroll up for older messages</span>
          </div>
        )}
        {messages.map((msg, idx) => {
          const prevRole = idx > 0 ? messages[idx - 1].info.role : null;
          const isContinuation = prevRole === msg.info.role;
          return (
            <MessageTurn
              key={`${msg.info.messageID || idx}-${idx}`}
              message={msg}
              isContinuation={isContinuation}
            />
          );
        })}
        {sessionStatus === "busy" && (
          <div className="message-thinking">
            <Loader2 size={16} className="spin" />
            <span>Thinking...</span>
          </div>
        )}
        <div ref={endRef} />
      </div>
    </div>
  );
}
