import React, { useRef, useEffect, useState, useCallback } from "react";
import type { Message } from "./types";
import { MessageTimeline } from "./MessageTimeline";
import { fetchSessionMessages } from "./api";
import { Loader2, CheckCircle2, XCircle, Bot, ExternalLink } from "lucide-react";

interface SubagentSessionProps {
  sessionId: string;
  title: string;
  /** SSE-driven messages from useSSE's subagentMessages map. */
  messages?: Message[];
  /** Whether the task tool is still running */
  isRunning?: boolean;
  /** Whether the task tool completed successfully */
  isCompleted?: boolean;
  /** Whether the task tool errored */
  isError?: boolean;
  /** Callback to navigate to this child session */
  onOpenSession?: (sessionId: string) => void;
}

/**
 * Renders a subagent session's messages inside a task accordion.
 *
 * Live sessions: messages arrive via SSE (subagentMessages map keyed by
 * the child session ID extracted from the task tool part's input.task_id).
 * Completed sessions (e.g. after page reload): fetches messages from
 * the REST API so historical task output is always visible.
 */
export const SubagentSession = React.memo(function SubagentSession({
  sessionId,
  title,
  messages: sseMessages,
  isRunning,
  isCompleted,
  isError,
  onOpenSession,
}: SubagentSessionProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // --- REST-fetched messages for completed/errored tasks with no SSE data ---
  const [fetchedMessages, setFetchedMessages] = useState<Message[] | null>(null);
  const [isFetching, setIsFetching] = useState(false);
  const fetchAttemptedRef = useRef(false);

  const hasSseMessages = sseMessages && sseMessages.length > 0;

  // Fetch from API when: task is done, no SSE messages, and we haven't tried yet
  const shouldFetch =
    !hasSseMessages && !isRunning && (isCompleted || isError) && !fetchAttemptedRef.current;

  const doFetch = useCallback(async () => {
    if (!sessionId) return;
    fetchAttemptedRef.current = true;
    setIsFetching(true);
    try {
      const resp = await fetchSessionMessages(sessionId);
      if (resp.messages.length > 0) {
        setFetchedMessages(resp.messages);
      }
    } catch (err) {
      console.warn("[SubagentSession] Failed to fetch messages for", sessionId, err);
    } finally {
      setIsFetching(false);
    }
  }, [sessionId]);

  useEffect(() => {
    if (shouldFetch) {
      doFetch();
    }
  }, [shouldFetch, doFetch]);

  // Use SSE messages when available, else fall back to fetched messages
  const messages = hasSseMessages ? sseMessages : fetchedMessages ?? undefined;
  const hasMessages = messages && messages.length > 0;

  // Auto-scroll to bottom when new messages arrive while running
  useEffect(() => {
    if (isRunning && scrollRef.current) {
      const el = scrollRef.current;
      const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
      if (isNearBottom) {
        el.scrollTop = el.scrollHeight;
      }
    }
  }, [messages, isRunning]);

  return (
    <div className={`subagent-session${isRunning ? " subagent-running" : ""}${isError ? " subagent-error-state" : ""}`}>
      <div className="subagent-header">
        <Bot size={14} className="subagent-icon" />
        <span className="subagent-title">{title}</span>
        <span className="subagent-status">
          {isRunning && (
            <>
              <span className="subagent-live-dot" />
              <Loader2 size={11} className="tool-spin-icon" />
              <span className="subagent-status-text">running</span>
            </>
          )}
          {isCompleted && (
            <>
              <CheckCircle2 size={12} className="tool-success-icon" />
              <span className="subagent-status-text">completed</span>
            </>
          )}
          {isError && (
            <>
              <XCircle size={12} className="tool-error-icon" />
              <span className="subagent-status-text">failed</span>
            </>
          )}
        </span>
        {onOpenSession && (
          <button
            className="subagent-open-link"
            onClick={(e) => {
              e.stopPropagation();
              onOpenSession(sessionId);
            }}
            title="Open this session"
          >
            <ExternalLink size={11} />
            <span>Open session</span>
          </button>
        )}
      </div>

      {isFetching ? (
        <div className="subagent-empty">
          <Loader2 size={14} className="tool-spin-icon" />
          <span>Loading task output...</span>
        </div>
      ) : !hasMessages ? (
        <div className="subagent-empty">
          {isRunning ? (
            <>
              <Loader2 size={14} className="tool-spin-icon" />
              <span>Subagent starting...</span>
            </>
          ) : (
            <span className="subagent-no-data">No task output available</span>
          )}
        </div>
      ) : (
        <div className="subagent-messages" ref={scrollRef}>
          <MessageTimeline
            messages={messages}
            sessionStatus={isRunning ? "busy" : "idle"}
            activeSessionId={sessionId}
            isLoadingMessages={false}
          />
        </div>
      )}
    </div>
  );
});
