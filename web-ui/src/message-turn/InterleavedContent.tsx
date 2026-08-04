import React, { useState } from "react";
import ReactMarkdown from "react-markdown";
import { ChevronDown, Layers3 } from "lucide-react";

import { ToolCall } from "../ToolCall";
import type { MessagePart, Message, SessionInfo } from "./types";
import { markdownComponents, REMARK_PLUGINS } from "./CodeBlock";
import { ThinkingAccordion } from "./ThinkingAccordion";

type ToolEntry = {
  part: MessagePart;
  childSession: SessionInfo | null;
};

function isToolPart(part: MessagePart): boolean {
  return part.type === "tool" || part.type === "tool-call" || part.type === "tool_call";
}

function toolType(part: MessagePart): string {
  const raw = part as MessagePart & Record<string, unknown>;
  const name = raw.tool || raw.toolName || raw.tool_name || raw.name || (raw.call as Record<string, unknown> | undefined)?.name;
  if (typeof name !== "string" || !name.trim()) {
    return "unknown:" + (part.id || part.callID || part.toolCallId || "part");
  }

  const normalized = name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_");
  const bashAliases = new Set(["bash", "shell", "sh", "zsh", "exec", "exec_command", "run_command", "command_execution", "terminal"]);
  const leaf = normalized.split("_").filter(Boolean).pop() || normalized;
  if (bashAliases.has(normalized) || bashAliases.has(leaf)) {
    return "bash";
  }
  return normalized;
}

function isTransparentPart(part: MessagePart): boolean {
  return ["step-start", "step-finish", "snapshot", "patch"].includes(part.type);
}

function toolLabel(name: string): string {
  const label = name.replace(/[_-]+/g, " ").trim();
  return label ? label.charAt(0).toUpperCase() + label.slice(1) : "Tool";
}

function renderTool(entry: ToolEntry, key: string, subagentMessages?: Map<string, Message[]>, onOpenSession?: (sessionId: string) => void) {
  return (
    <ToolCall
      key={key}
      part={entry.part}
      childSession={entry.childSession}
      subagentMessages={subagentMessages}
      onOpenSession={onOpenSession}
    />
  );
}

function ConsecutiveToolGroup({
  entries,
  type,
  subagentMessages,
  onOpenSession,
}: {
  entries: ToolEntry[];
  type: string;
  subagentMessages?: Map<string, Message[]>;
  onOpenSession?: (sessionId: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const label = toolLabel(type);
  const firstKey = entries[0].part.callID || entries[0].part.toolCallId || `tool-group-${type}`;

  if (entries.length < 2) {
    return renderTool(entries[0], firstKey, subagentMessages, onOpenSession);
  }

  return (
    <div className={`consecutive-tool-group${expanded ? " is-expanded" : ""}`}>
      <div className="consecutive-tool-group-main">
        {renderTool(entries[0], firstKey, subagentMessages, onOpenSession)}
      </div>
      <button
        type="button"
        className="consecutive-tool-group-toggle"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        <Layers3 size={13} />
        <span>{expanded ? "Collapse" : `+${entries.length - 1} more ${label} calls`}</span>
        <ChevronDown size={13} className="consecutive-tool-group-chevron" />
      </button>
      {expanded && (
        <div className="consecutive-tool-group-items">
          {entries.slice(1).map((entry, index) => renderTool(
            entry,
            entry.part.callID || entry.part.toolCallId || `${firstKey}-${index + 1}`,
            subagentMessages,
            onOpenSession,
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Render parts in order while preserving text, reasoning, and tool boundaries.
 * Task calls still consume child sessions while the group is being built.
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
      </div>,
    );
    currentTextChunks = [];
  }

  for (let index = 0; index < allParts.length;) {
    const part = allParts[index].part;
    if (part.type === "text" && part.text) {
      currentTextChunks.push(part.text);
      index++;
      continue;
    }

    if (part.type === "reasoning" || part.type === "thinking" || part.type === "analysis") {
      flushText();
      const reasoning: string[] = [];
      while (index < allParts.length && (allParts[index].part.type === "reasoning" || allParts[index].part.type === "thinking" || allParts[index].part.type === "analysis")) {
        const raw = allParts[index].part as MessagePart & Record<string, unknown>;
        const value = raw.text || raw.reasoning || raw.thinking || raw.analysis;
        if (typeof value === "string") reasoning.push(value);
        index++;
      }
      const text = reasoning.join("\n\n");
      if (part.type === "reasoning") {
        elements.push(<div className="message-body" key={"reasoning-" + index}><ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{text}</ReactMarkdown></div>);
      } else {
        elements.push(<ThinkingAccordion key={"thinking-" + index} text={text} />);
      }
      continue;
    }

    if (!isToolPart(part)) {
      index++;
      continue;
    }

    flushText();
    const type = toolType(part);
    const entries: ToolEntry[] = [];

    while (index < allParts.length) {
      const current = allParts[index].part;
      if (isTransparentPart(current)) {
        index++;
        continue;
      }
      if (!isToolPart(current) || toolType(current) !== type) break;
      const isTask = toolType(current) === "task";
      entries.push({
        part: current,
        childSession: isTask ? childSessions[taskToolIndex] ?? null : null,
      });
      if (isTask) taskToolIndex++;
      index++;
    }

    elements.push(
      <ConsecutiveToolGroup
        key={entries[0].part.callID || entries[0].part.toolCallId || `tool-${key++}`}
        entries={entries}
        type={type}
        subagentMessages={subagentMessages}
        onOpenSession={onOpenSession}
      />,
    );
  }

  flushText();
  return elements;
}
