import React, { useMemo, useCallback, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";

import type { Message, MessagePart, ModelRef } from "./types";
import type { MessageGroup } from "./MessageTimeline";
import type { SessionInfo } from "./api";
import { ToolCall } from "./ToolCall";
import { User, Bot, Wrench, Copy, Check, RotateCcw, WrapText, Download, Bookmark } from "lucide-react";

interface Props {
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
}

/** Render a model reference as a display string. */
function modelLabel(m: ModelRef): string {
  if (typeof m === "string") return m;
  return m.modelID || JSON.stringify(m);
}

/** File extension mapping for common languages */
const LANG_EXTENSIONS: Record<string, string> = {
  javascript: "js", typescript: "ts", python: "py", rust: "rs", ruby: "rb",
  go: "go", java: "java", kotlin: "kt", swift: "swift", csharp: "cs",
  cpp: "cpp", c: "c", html: "html", css: "css", json: "json", yaml: "yml",
  toml: "toml", markdown: "md", bash: "sh", shell: "sh", sql: "sql",
  xml: "xml", jsx: "jsx", tsx: "tsx", php: "php", lua: "lua", zig: "zig",
};

/** Interactive code block with line numbers, word wrap, copy, and download */
function CodeBlock({ language, code }: { language: string; code: string }) {
  const [copied, setCopied] = useState(false);
  const [wordWrap, setWordWrap] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [code]);

  const handleDownload = useCallback(() => {
    const ext = LANG_EXTENSIONS[language] || "txt";
    const blob = new Blob([code], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `snippet.${ext}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [code, language]);

  // Generate line numbers
  const lineCount = code.split("\n").length;
  const lineNumbers = Array.from({ length: lineCount }, (_, i) => i + 1);

  return (
    <div className={`code-block-wrapper ${wordWrap ? "code-block-wrap" : ""}`}>
      <div className="code-block-header">
        <span>{language}</span>
        <div className="code-block-actions">
          <button
            className={`code-block-action-btn ${wordWrap ? "active" : ""}`}
            onClick={() => setWordWrap((v) => !v)}
            aria-label="Toggle word wrap"
            title="Toggle word wrap"
          >
            <WrapText size={12} />
          </button>
          <button
            className="code-block-action-btn"
            onClick={handleDownload}
            aria-label="Download code"
            title="Download"
          >
            <Download size={12} />
          </button>
          <button
            className="code-block-action-btn"
            onClick={handleCopy}
            aria-label="Copy code"
            title="Copy"
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
          </button>
        </div>
      </div>
      <div className="code-block-body">
        <div className="code-block-line-numbers" aria-hidden="true">
          {lineNumbers.map((n) => (
            <span key={n}>{n}</span>
          ))}
        </div>
        <SyntaxHighlighter
          useInlineStyles={false}
          language={language}
          PreTag="div"
          customStyle={{
            margin: 0,
            padding: 0,
            borderRadius: 0,
            background: "transparent",
            whiteSpace: "pre",
            overflowX: wordWrap ? "visible" : "auto",
            flex: 1,
            minWidth: 0,
          }}
        >
          {code}
        </SyntaxHighlighter>
      </div>
    </div>
  );
}

/** Markdown renderer components (shared, no need to recreate per render). */
const markdownComponents = {
  code(props: React.HTMLAttributes<HTMLElement> & { children?: React.ReactNode }) {
    const { className, children, ...rest } = props;
    const match = /language-(\w+)/.exec(className || "");
    const codeStr = String(children).replace(/\n$/, "");
    if (match) {
      return <CodeBlock language={match[1]} code={codeStr} />;
    }
    return (
      <code className="inline-code" {...rest}>
        {children}
      </code>
    );
  },
};

export const MessageTurn = React.memo(function MessageTurn({ group, childSessions, onRetry, subagentMessages, searchMatchIds, activeSearchMatchId, isBookmarked, onToggleBookmark, sessionId, onOpenSession }: Props) {
  const { role, messages } = group;
  const [copied, setCopied] = useState(false);

  if (role === "system") return null;

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

  // Collect model info from the first message that has it
  const headerModel = useMemo(() => {
    for (const msg of messages) {
      if (msg.info.model) return msg.info.model;
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

    // Walk parts in order; consecutive text parts get joined,
    // but we keep them as separate segments so text before/after tools stays ordered
    let currentTextChunks: string[] = [];

    for (const { part } of allParts) {
      if (part.type === "text" && part.text) {
        currentTextChunks.push(part.text);
      } else if (part.type === "tool" || part.type === "tool-call" || part.type === "tool_call") {
        // Flush any pending text
        if (currentTextChunks.length > 0) {
          texts.push(currentTextChunks.join("\n"));
          currentTextChunks = [];
        }
        tools.push({ part, idx: toolIdx++ });
      }
    }
    // Flush remaining text
    if (currentTextChunks.length > 0) {
      texts.push(currentTextChunks.join("\n"));
    }

    return { textSegments: texts, toolParts: tools };
  }, [allParts]);

  // If there are no tool parts, just combine all text into one block
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

  return (
    <div className={`message-turn message-turn-${role}${isOptimistic ? " message-turn-optimistic" : ""}${isSearchMatch ? " message-turn-search-match" : ""}${isActiveMatch ? " message-turn-active-match" : ""}`}>
      {/* Avatar */}
      <div className={`message-avatar ${role}`}>
        {isUser ? <User size={16} /> : isAssistant ? <Bot size={16} /> : <Wrench size={16} />}
      </div>

      {/* Content */}
      <div className="message-content">
        {/* Header */}
        <div className="message-header">
          <span className="message-role">
            {isUser ? "You" : isAssistant ? "Assistant" : role}
          </span>
          {isOptimistic && (
            <span className="message-sending-badge">Sending...</span>
          )}
          {headerModel && (
            <span className="message-model">{modelLabel(headerModel)}</span>
          )}
          {totalCost > 0 && (
            <span className="message-cost">
              ${totalCost.toFixed(4)}
            </span>
          )}
        </div>

        {/* Content: render in order */}
        {hasMixedContent ? (
          <>
            {/* Interleaved text and tool parts - render all text segments
                then all tool calls (tool calls usually come between text) */}
            {renderInterleavedContent(allParts, childSessions || [], subagentMessages, onOpenSession)}
          </>
        ) : (
          /* Simple case: all text, no tools */
          textSegments.length > 0 && (
            <div className="message-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                {textSegments.join("\n")}
              </ReactMarkdown>
            </div>
          )
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
    // Ignore step-start/step-finish for now
  }

  flushText();
  return elements;
}
