import React from "react";
import ReactMarkdown from "react-markdown";

import { ToolCall } from "../ToolCall";
import type { MessagePart, Message, SessionInfo } from "./types";
import { markdownComponents, REMARK_PLUGINS } from "./CodeBlock";

/**
 * Render parts in order, grouping consecutive text parts together
 * and rendering tool calls inline between text blocks.
 *
 * For "task" tool calls, we match child sessions by order: the N-th task tool
 * gets the N-th child session (sorted by creation time).
 */
export function renderInterleavedContent(
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
    if (currentTextChunks.length === 0) return;
    const text = currentTextChunks.join("\n");
    elements.push(
      <div className="message-body" key={`text-${key++}`}>
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
          {text}
        </ReactMarkdown>
      </div>
    );
    currentTextChunks = [];
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
