import React, { useMemo, useCallback, useState } from "react";
import ReactMarkdown from "react-markdown";
import { User, Bot, Wrench, Copy, Check, RotateCcw, Bookmark, AlertTriangle, Brain, ChevronRight, FileText, ArrowLeftRight } from "lucide-react";

import type { MessageTurnProps, MessagePart } from "./types";
import { modelLabel, parseMemoryBlock, parseFileContext, parseHandoffBlock } from "./helpers";
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
  userTurnStates,
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

  // Badge state comes from the timeline, which can see the whole transcript and
  // so knows whether this turn is the one being answered or is waiting behind it.
  const turnState = useMemo(() => {
    if (!isUser || !userTurnStates?.size) return null;
    for (const msg of messages) {
      const state = userTurnStates.get(msg.info.messageID || msg.info.id || "");
      if (state) return state;
    }
    return null;
  }, [isUser, userTurnStates, messages]);

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

  // Separate text, tool, and file/image parts
  const { textSegments, toolParts, fileParts } = useMemo(() => {
    const texts: string[] = [];
    const tools: { part: MessagePart; idx: number }[] = [];
    const files: MessagePart[] = [];
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
      } else if (part.type === "file" && part.url && part.mime?.startsWith("image/")) {
        files.push(part);
      }
    }
    if (currentTextChunks.length > 0) {
      texts.push(currentTextChunks.join("\n"));
    }

    return { textSegments: texts, toolParts: tools, fileParts: files };
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

  // Parse the runner-handoff block. It fences the prior runner's transcript,
  // which the new runner needs but the user never typed.
  const handoffBlock = useMemo(() => {
    if (!isUser) return null;
    return parseHandoffBlock(textAfterFiles);
  }, [isUser, textAfterFiles]);

  // Instructions sit inside the handoff block's user half when both are present.
  const textAfterHandoff = handoffBlock ? handoffBlock.userText : textAfterFiles;

  // Parse memory block from user messages (after stripping file + handoff)
  const memoryBlock = useMemo(() => {
    if (!isUser) return null;
    return parseMemoryBlock(textAfterHandoff);
  }, [isUser, textAfterHandoff]);

  // Display text: stripped of file blocks, handoff transcript and memory block
  const displaySegments = useMemo(() => {
    if (memoryBlock) return [memoryBlock.userText];
    if (handoffBlock) return [handoffBlock.userText];
    if (fileContext) return [fileContext.userText];
    return textSegments;
  }, [textSegments, memoryBlock, handoffBlock, fileContext]);

  const [memoryOpen, setMemoryOpen] = useState(false);
  const [handoffOpen, setHandoffOpen] = useState(false);

  // A handoff turn's raw text is mostly the prior transcript. Copying or
  // retrying that would hand the whole blob back, so those act on what the
  // user actually wrote.
  const actionText = handoffBlock ? displaySegments.join("\n").trim() : plainText;

  const handleCopy = useCallback(() => {
    if (!actionText) return;
    navigator.clipboard.writeText(actionText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [actionText]);

  const handleRetry = useCallback(() => {
    if (isUser && actionText && onRetry) {
      onRetry(actionText);
    }
  }, [isUser, actionText, onRetry]);

  if (role === "system") {
    // Compaction summary card: the completed counterpart to the live "Compacting…" banner.
    const compact = messages.find((m) => m.info.variant === "compact");
    if (compact) {
      const pre = compact.info.preTokens ?? 0;
      const dur = compact.info.durationMs ?? 0;
      const trigger = compact.info.trigger === "auto" ? "auto" : "manual";
      const tokens =
        pre >= 1_000_000
          ? `${(pre / 1_000_000).toFixed(1)}M`
          : pre >= 1000
            ? `${Math.round(pre / 1000)}K`
            : `${pre}`;
      const secs = Math.round(dur / 1000);
      const took = secs >= 60 ? `${Math.floor(secs / 60)}m ${secs % 60}s` : `${secs}s`;
      const bits: string[] = [];
      if (pre > 0) bits.push(`${tokens} tokens condensed`);
      if (secs > 0) bits.push(`${took}`);
      bits.push(trigger);
      return (
        <div className="message-turn message-turn-notification message-turn-sys-compact">
          <div className="notification-bubble notification-compact">
            <span className="notification-icon">🗜️</span>
            <span className="notification-text">
              <strong>Conversation compacted</strong>
              {bits.length ? <span className="compaction-summary-meta"> · {bits.join(" · ")}</span> : null}
            </span>
          </div>
        </div>
      );
    }
    // System bubbles: opman-injected notifications (task-notifications, reminders) and
    // claude's own surfaced system messages (info / warning / error). Anything without a
    // recognized variant stays hidden.
    const variant = (messages
      .map((m) => m.info.variant as string | undefined)
      .find((v) => v === "notification" || v === "warning" || v === "error")) as
      | "notification" | "warning" | "error" | undefined;
    if (!variant) return null;
    const noteText = messages
      .flatMap((m) => m.parts)
      .filter((p) => p.type === "text" && p.text)
      .map((p) => p.text as string)
      .join("\n")
      .trim();
    if (!noteText) return null;
    const icon = variant === "error" ? "⛔" : variant === "warning" ? "⚠️" : "⚙";
    return (
      <div className={`message-turn message-turn-notification message-turn-sys-${variant}`}>
        <div className={`notification-bubble notification-${variant}`}>
          <span className="notification-icon">{icon}</span>
          <span className="notification-text">{noteText}</span>
        </div>
      </div>
    );
  }

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
            {isUser ? "You" : isAssistant ? (headerAgent ? headerAgent + " agent" : "Assistant") : role}
          </span>
          {turnState === "sending" && (
            <span className="message-sending-badge">Sending...</span>
          )}
          {turnState === "queued" && (
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
          {handoffBlock && (
            <button
              className={`memory-header-toggle handoff-header-toggle${handoffOpen ? " open" : ""}`}
              onClick={() => setHandoffOpen((o) => !o)}
            >
              <ChevronRight size={12} className="memory-header-chevron" />
              <ArrowLeftRight size={12} />
              <span>
                {handoffBlock.fromRunner
                  ? `Handed off from ${handoffBlock.fromRunner}`
                  : "Handed off"}
              </span>
            </button>
          )}
          {memoryBlock && (
            <button
              className={`memory-header-toggle${memoryOpen ? " open" : ""}`}
              onClick={() => setMemoryOpen((o) => !o)}
            >
              <ChevronRight size={12} className="memory-header-chevron" />
              <Brain size={12} />
              <span>{memoryBlock.items.length === 1 ? "Session instruction" : "Session instructions"}</span>
            </button>
          )}
        </div>

        {/* Handoff transcript (collapsed by default — it is context, not a message) */}
        {handoffBlock && handoffOpen && (
          <pre className="memory-accordion-body handoff-accordion-body">
            {handoffBlock.transcript}
          </pre>
        )}

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

        {/* Image attachments */}
        {fileParts.length > 0 && (
          <div className="message-image-attachments">
            {fileParts.map((p, i) => (
              <div key={i} className="message-image-thumb">
                <img src={p.url} alt={p.filename || "attachment"} loading="lazy" />
                {p.filename && <span className="message-image-name">{p.filename}</span>}
              </div>
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
