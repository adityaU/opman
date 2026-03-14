import React, { useMemo, useCallback, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { User, Bot, Wrench, Copy, Check, RotateCcw, Bookmark, AlertTriangle } from "lucide-react";

import { ToolCall } from "../ToolCall";
import type { MessageTurnProps, MessagePart, Message, SessionInfo } from "./types";
import { modelLabel } from "./helpers";
import { markdownComponents } from "./CodeBlock";
import { agentColor } from "../utils/theme";

export const MessageTurn = React.memo(function MessageTurn({
  group,
  childSessions,
  onRetry,
  subagentMessages,
  searchMatchIds,
  activeSearchMatchId,
  isBookmarked,
  onToggleBookmark,
  sessionId,
  onOpenSession,
  pendingAssistantId,
}: MessageTurnProps) {
  const { role, messages } = group;
  const [copied, setCopied] = useState(false);

  const isUser = role === "user";
  const isAssistant = role === "assistant";

  // Get first message ID for bookmark and search
  const firstMsgId = messages[0]?.info.messageID || messages[0]?.info.id || "";

  // Check if any message in the group is a search match
  const isSearchMatch = useMemo(() => {
    if (!searchMatchIds || searchMatchIds.size === 0) return false;
    return messages.some((msg) => {
      const id = msg.info.messageID || msg.info.id || "";
      return searchMatchIds.has(id);
    });
  }, [messages, searchMatchIds]);

  // Check if this is the active search match
  const isActiveMatch = useMemo(() => {
    if (!activeSearchMatchId) return false;
    return messages.some((msg) => {
      const id = msg.info.messageID || msg.info.id || "";
      return id === activeSearchMatchId;
    });
  }, [messages, activeSearchMatchId]);

  // Bookmark state
  const bookmarked = isBookmarked ? isBookmarked(firstMsgId) : false;

  const handleToggleBookmark = useCallback(() => {
    if (!onToggleBookmark || !firstMsgId) return;
    // Get first text for preview
    let preview = "";
    for (const msg of messages) {
      for (const part of msg.parts) {
        if (part.text) { preview = part.text; break; }
      }
      if (preview) break;
    }
    onToggleBookmark(firstMsgId, sessionId || "", role, preview);
  }, [onToggleBookmark, firstMsgId, sessionId, role, messages]);

  // Detect if this group contains an optimistic (pending) message
  const isOptimistic = useMemo(() => {
    return messages.some((msg) => {
      const id = msg.info.messageID || msg.info.id || "";
      return id.startsWith("__optimistic__");
    });
  }, [messages]);

  // A user message is "queued" if the session is still processing an earlier
  // assistant message (pendingAssistantId) and this user message was sent after
  // that assistant message (its ID sorts after the pending assistant ID).
  const isQueued = useMemo(() => {
    if (!isUser || !pendingAssistantId) return false;
    return messages.some((msg) => {
      const id = msg.info.messageID || msg.info.id || "";
      return id > pendingAssistantId;
    });
  }, [isUser, pendingAssistantId, messages]);

  // Collect model info from the first message that has it
  const headerModel = useMemo(() => {
    for (const msg of messages) {
      if (msg.info.model) return msg.info.model;
    }
    return null;
  }, [messages]);

  // Collect agent info from the first message that has it
  const headerAgent = useMemo(() => {
    for (const msg of messages) {
      if (msg.info.agent) return msg.info.agent;
    }
    return null;
  }, [messages]);

  // Sum total cost across all messages in group
  const totalCost = useMemo(() => {
    let sum = 0;
    for (const msg of messages) {
      if (msg.metadata?.cost) sum += msg.metadata.cost;
    }
    return sum;
  }, [messages]);

  // Extract error from any message in the group
  const errorText = useMemo(() => {
    for (const msg of messages) {
      if (msg.info.error) {
        if (typeof msg.info.error === "string") return msg.info.error;
        if (typeof msg.info.error === "object" && msg.info.error !== null) {
          const e = msg.info.error as Record<string, unknown>;
          return (e.message || e.error || JSON.stringify(msg.info.error)) as string;
        }
        return String(msg.info.error);
      }
    }
    return null;
  }, [messages]);

  // Flatten all parts from all messages in the group, keeping order
  const allParts = useMemo(() => {
    const parts: { part: MessagePart; msgIdx: number }[] = [];
    messages.forEach((msg, msgIdx) => {
      for (const part of msg.parts) {
        parts.push({ part, msgIdx });
      }
    });
    return parts;
  }, [messages]);

  // Separate text and tool parts
  const { textSegments, toolParts } = useMemo(() => {
    const texts: string[] = [];
    const tools: { part: MessagePart; idx: number }[] = [];
    let toolIdx = 0;

    let currentTextChunks: string[] = [];

    for (const { part } of allParts) {
      if (part.type === "text" && part.text) {
        currentTextChunks.push(part.text);
      } else if (part.type === "tool" || part.type === "tool-call" || part.type === "tool_call") {
        if (currentTextChunks.length > 0) {
          texts.push(currentTextChunks.join("\n"));
          currentTextChunks = [];
        }
        tools.push({ part, idx: toolIdx++ });
      }
    }
    if (currentTextChunks.length > 0) {
      texts.push(currentTextChunks.join("\n"));
    }

    return { textSegments: texts, toolParts: tools };
  }, [allParts]);

  const hasMixedContent = toolParts.length > 0;

  // Extract plain text for copy action
  const plainText = useMemo(() => {
    return textSegments.join("\n").trim();
  }, [textSegments]);

  const handleCopy = useCallback(() => {
    if (!plainText) return;
    navigator.clipboard.writeText(plainText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [plainText]);

  const handleRetry = useCallback(() => {
    if (isUser && plainText && onRetry) {
      onRetry(plainText);
    }
  }, [isUser, plainText, onRetry]);

  if (role === "system") return null;

  return (
    <div className={`message-turn message-turn-${role}${isOptimistic ? " message-turn-optimistic" : ""}${isSearchMatch ? " message-turn-search-match" : ""}${isActiveMatch ? " message-turn-active-match" : ""}`}>
      {/* Content */}
      <div className="message-content">
        {/* Header — avatar is inline with the role label */}
        <div className="message-header">
          <div className={`message-avatar ${role}`}>
            {isUser ? <User size={16} /> : isAssistant ? <Bot size={16} /> : <Wrench size={16} />}
          </div>
          <span className="message-role">
            {isUser ? "You" : isAssistant ? "Assistant" : role}
          </span>
          {isOptimistic && !isQueued && (
            <span className="message-sending-badge">Sending...</span>
          )}
          {isQueued && (
            <span className="message-queued-badge">Queued</span>
          )}
          {headerModel && (
            <span className="message-model">{modelLabel(headerModel)}</span>
          )}
          {totalCost > 0 && (
            <span className="message-cost">
              ${totalCost.toFixed(4)}
            </span>
          )}
          {isAssistant && headerAgent && (
            <span className="message-agent" style={{
              color: agentColor(headerAgent),
              borderColor: `color-mix(in srgb, ${agentColor(headerAgent)} 25%, transparent)`,
              backgroundColor: `color-mix(in srgb, ${agentColor(headerAgent)} 10%, transparent)`,
            }}>{headerAgent}</span>
          )}
        </div>

        {/* Content: render in order */}
        {hasMixedContent ? (
          <>
            {renderInterleavedContent(allParts, childSessions || [], subagentMessages, onOpenSession)}
          </>
        ) : (
          textSegments.length > 0 && (
            <div className="message-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                {textSegments.join("\n")}
              </ReactMarkdown>
            </div>
          )
        )}

        {/* Error banner */}
        {errorText && (
          <div className="message-error-banner">
            <AlertTriangle size={14} />
            <span>{errorText}</span>
          </div>
        )}

        {/* Action bar — shown on hover */}
        {!isOptimistic && (
          <div className="message-actions">
            {firstMsgId && onToggleBookmark && (
              <button
                className={`message-action-btn ${bookmarked ? "bookmarked" : ""}`}
                onClick={handleToggleBookmark}
                aria-label={bookmarked ? "Remove bookmark" : "Bookmark message"}
                title={bookmarked ? "Remove bookmark" : "Bookmark message"}
              >
                <Bookmark size={13} fill={bookmarked ? "currentColor" : "none"} />
              </button>
            )}
            {plainText && (
              <button
                className="message-action-btn"
                onClick={handleCopy}
                aria-label="Copy message"
                title="Copy message"
              >
                {copied ? <Check size={13} /> : <Copy size={13} />}
              </button>
            )}
            {isUser && onRetry && plainText && (
              <button
                className="message-action-btn"
                onClick={handleRetry}
                aria-label="Retry message"
                title="Retry message"
              >
                <RotateCcw size={13} />
              </button>
            )}
            {isAssistant && headerModel && (
              <span className="message-actions-model">{modelLabel(headerModel)}</span>
            )}
          </div>
        )}
      </div>
    </div>
  );
});

/**
 * Render parts in order, grouping consecutive text parts together
 * and rendering tool calls inline between text blocks.
 *
 * For "task" tool calls, we match child sessions by order: the N-th task tool
 * gets the N-th child session (sorted by creation time).
 */
function renderInterleavedContent(
  allParts: { part: MessagePart; msgIdx: number }[],
  childSessions: SessionInfo[],
  subagentMessages?: Map<string, Message[]>,
  onOpenSession?: (sessionId: string) => void,
) {
  const elements: React.ReactNode[] = [];
  let currentTextChunks: string[] = [];
  let key = 0;
  let taskToolIndex = 0;

  function flushText() {
    if (currentTextChunks.length > 0) {
      const text = currentTextChunks.join("\n");
      elements.push(
        <div className="message-body" key={`text-${key++}`}>
          <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
            {text}
          </ReactMarkdown>
        </div>
      );
      currentTextChunks = [];
    }
  }

  for (const { part } of allParts) {
    if (part.type === "text" && part.text) {
      currentTextChunks.push(part.text);
    } else if (part.type === "tool" || part.type === "tool-call" || part.type === "tool_call") {
      flushText();
      const toolName = part.tool || part.toolName || "";
      const isTask = toolName === "task";
      const matched = isTask ? childSessions[taskToolIndex] ?? null : null;
      if (isTask) taskToolIndex++;

      elements.push(
        <ToolCall
          key={part.callID || part.toolCallId || `tool-${key++}`}
          part={part}
          childSession={matched}
          subagentMessages={subagentMessages}
          onOpenSession={onOpenSession}
        />
      );
    }
  }

  flushText();
  return elements;
}
