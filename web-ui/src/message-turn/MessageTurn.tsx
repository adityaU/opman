import React, { useMemo, useCallback, useState } from "react";
import ReactMarkdown from "react-markdown";
import { User, Bot, Wrench, Copy, Check, RotateCcw, Bookmark, AlertTriangle, Brain, ChevronRight, FileText } from "lucide-react";

import type { MessageTurnProps, MessagePart } from "./types";
import { modelLabel, parseMemoryBlock, parseFileContext } from "./helpers";
import { markdownComponents, REMARK_PLUGINS } from "./CodeBlock";
import { agentColor } from "../utils/theme";
import { renderInterleavedContent } from "./InterleavedContent";

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
  const firstMsgId = messages[0]?.info.messageID || messages[0]?.info.id || "";

  const msgId = (m: typeof messages[0]) => m.info.messageID || m.info.id || "";
  const isSearchMatch = !!searchMatchIds?.size && messages.some((m) => searchMatchIds.has(msgId(m)));
  const isActiveMatch = !!activeSearchMatchId && messages.some((m) => msgId(m) === activeSearchMatchId);

  // Bookmark state
  const bookmarked = isBookmarked ? isBookmarked(firstMsgId) : false;

  const handleToggleBookmark = useCallback(() => {
    if (!onToggleBookmark || !firstMsgId) return;
    let preview = "";
    for (const msg of messages) { for (const part of msg.parts) { if (part.text) { preview = part.text; break; } } if (preview) break; }
    onToggleBookmark(firstMsgId, sessionId || "", role, preview);
  }, [onToggleBookmark, firstMsgId, sessionId, role, messages]);

  // Detect if this group contains an optimistic (pending) message
  const isOptimistic = messages.some((msg) => (msg.info.messageID || msg.info.id || "").startsWith("__optimistic__"));

  // Queued: session is still processing an earlier assistant message
  const isQueued = isUser && !!pendingAssistantId &&
    messages.some((msg) => (msg.info.messageID || msg.info.id || "") > pendingAssistantId);

  // Collect model/agent/cost from messages
  const headerModel = useMemo(() => {
    for (const msg of messages) if (msg.info.model) return msg.info.model;
    return null;
  }, [messages]);
  const headerAgent = messages.find((m) => m.info.agent)?.info.agent ?? null;
  const totalCost = messages.reduce((s, m) => s + (m.metadata?.cost || 0), 0);

  // Extract error from any message
  const errorText = useMemo(() => {
    for (const msg of messages) {
      if (!msg.info.error) continue;
      if (typeof msg.info.error === "string") return msg.info.error;
      const e = msg.info.error as Record<string, unknown>;
      return (e.message || e.error || JSON.stringify(msg.info.error)) as string;
    }
    return null;
  }, [messages]);

  // Flatten all parts from all messages in the group, keeping order
  const allParts = useMemo(() => {
    const parts: { part: MessagePart; msgIdx: number }[] = [];
    messages.forEach((msg, msgIdx) => { for (const part of msg.parts) parts.push({ part, msgIdx }); });
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
  const plainText = textSegments.join("\n").trim();

  // Parse file context (@file mentions) from user messages
  const fileContext = useMemo(() => {
    if (!isUser) return null;
    return parseFileContext(plainText);
  }, [isUser, plainText]);

  // Text with file blocks stripped (for memory parsing + display)
  const textAfterFiles = useMemo(() => {
    return fileContext ? fileContext.userText : plainText;
  }, [fileContext, plainText]);

  // Parse memory block from user messages (after stripping file context)
  const memoryBlock = useMemo(() => {
    if (!isUser) return null;
    return parseMemoryBlock(textAfterFiles);
  }, [isUser, textAfterFiles]);

  // Display text: stripped of file blocks + memory block
  const displaySegments = useMemo(() => {
    if (memoryBlock) return [memoryBlock.userText];
    if (fileContext) return [fileContext.userText];
    return textSegments;
  }, [textSegments, memoryBlock, fileContext]);

  const [memoryOpen, setMemoryOpen] = useState(false);

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
          {memoryBlock && (
            <button
              className={`memory-header-toggle${memoryOpen ? " open" : ""}`}
              onClick={() => setMemoryOpen((o) => !o)}
            >
              <ChevronRight size={12} className="memory-header-chevron" />
              <Brain size={12} />
              <span>{memoryBlock.items.length} {memoryBlock.items.length === 1 ? "memory" : "memories"}</span>
            </button>
          )}
        </div>

        {/* Memory accordion body (expands below header) */}
        {memoryBlock && memoryOpen && (
          <div className="memory-accordion-body">
            {memoryBlock.items.map((item, i) => (
              <div key={i} className="memory-accordion-item">
                <span className="memory-accordion-item-label">{item.label}</span>
                {item.content && (
                  <span className="memory-accordion-item-content">{item.content}</span>
                )}
              </div>
            ))}
          </div>
        )}

        {/* File context pills for user messages with @file mentions */}
        {fileContext && fileContext.paths.length > 0 && (
          <div className="file-context-pills">
            {fileContext.paths.map((p) => (
              <span key={p} className="file-context-pill">
                <FileText size={11} />
                <span className="file-context-pill-path">{p}</span>
              </span>
            ))}
          </div>
        )}

        {/* Content: render in order */}
        {hasMixedContent ? (
          <>
            {renderInterleavedContent(allParts, childSessions || [], subagentMessages, onOpenSession)}
          </>
        ) : (
          displaySegments.length > 0 && (
            <div className="message-body">
              <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                {displaySegments.join("\n")}
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
