import React from "react";
import type { Message } from "./types";
import { MessageTimeline } from "./MessageTimeline";

interface SubagentSessionProps {
  sessionId: string;
  title: string;
  /** SSE-driven messages from useSSE's subagentMessages map. */
  messages?: Message[];
}

/**
 * Renders a subagent session's messages inside a task accordion.
 * Purely SSE-driven — no REST fetching or polling.
 * Messages arrive via `message.updated`, `message.part.updated`, and
 * `message.part.delta` SSE events routed through useSSE's subagentMapsRef.
 */
export const SubagentSession = React.memo(function SubagentSession({
  sessionId,
  title,
  messages,
}: SubagentSessionProps) {
  const hasMessages = messages && messages.length > 0;

  if (!hasMessages) {
    return (
      <div className="subagent-empty">
        <span>Subagent starting...</span>
      </div>
    );
  }

  return (
    <div className="subagent-session">
      <div className="subagent-header">
        <span className="subagent-title">{title}</span>
        <span className="subagent-message-count">
          {messages.length} message{messages.length !== 1 ? "s" : ""}
          <span className="subagent-live-badge"> live</span>
        </span>
      </div>
      <div className="subagent-messages">
        <MessageTimeline
          messages={messages}
          sessionStatus="busy"
          activeSessionId={null}
          isLoadingMessages={false}
        />
      </div>
    </div>
  );
});
