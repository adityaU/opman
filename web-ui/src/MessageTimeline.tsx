import React, { useRef, useEffect, useCallback } from "react";
import type { Message } from "./types";
import { MessageTurn } from "./MessageTurn";
import { Loader2 } from "lucide-react";

interface Props {
  messages: Message[];
  sessionStatus: "idle" | "busy";
  activeSessionId: string | null;
}

export function MessageTimeline({
  messages,
  sessionStatus,
  activeSessionId,
}: Props) {
  const endRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  /** Tracks whether we should auto-scroll to bottom (user is near bottom). */
  const shouldAutoScrollRef = useRef(true);

  // Auto-scroll to bottom on new messages (only if user was already near bottom)
  useEffect(() => {
    if (shouldAutoScrollRef.current) {
      endRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages.length, sessionStatus]);

  // Scroll listener: track if user is near bottom for auto-scroll decision
  const handleScroll = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    const distFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight;
    shouldAutoScrollRef.current = distFromBottom < 100;
  }, []);

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
