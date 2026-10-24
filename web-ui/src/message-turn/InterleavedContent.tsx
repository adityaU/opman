import React, { useState } from "react";
import ReactMarkdown from "react-markdown";
import { ChevronDown, Layers3 } from "lucide-react";

import { ToolCall } from "../ToolCall";
import type { MessagePart, Message, SessionInfo } from "./types";
import { markdownComponents, REMARK_PLUGINS } from "./CodeBlock";
import { ThinkingAccordion } from "./ThinkingAccordion";
import { formatDuration } from "../tool-call/helpers";
import { isToolPart, toolDisplayName, toolType } from "./toolLabels";
import { nestToolParts } from "./nestToolParts";
import { useThinkingVisibility } from "../utils/thinkingVisibility";

type ToolEntry = {
  part: MessagePart;
  childSession: SessionInfo | null;
};

function ReasoningBlock({ text }: { text: string }) {
  const showThinking = useThinkingVisibility();
  if (!showThinking) return null;
  return (
    <div className="message-body">
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
        {text}
      </ReactMarkdown>
    </div>
  );
}


/**
 * The prose a soft part carries, or `null` if the part is not a prose part.
 *
 * Empty is meaningfully different from absent here: the runner emits a text or
 * thinking block for every one the model produced, including the empty ones.
 */
function softText(part: MessagePart): string | null {
  const raw = part as MessagePart & Record<string, unknown>;
  if (part.type === "text") return typeof raw.text === "string" ? raw.text : "";
  if (part.type === "reasoning" || part.type === "thinking" || part.type === "analysis") {
    const value = raw.text || raw.reasoning || raw.thinking || raw.analysis;
    return typeof value === "string" ? value : "";
  }
  return null;
}

/**
 * Parts that put nothing on screen between the parts around them.
 *
 * This is what decides whether a run of tool calls stays together, so "renders
 * nothing" is the only workable test. Two consecutive Bash calls used to group
 * or not depending on whether the model happened to emit an empty `thinking`
 * block before the second one — invisible in the transcript, so the grouping
 * looked arbitrary. A tool result is the other half of a call, never a divider.
 */
function isTransparentPart(part: MessagePart): boolean {
  if (["step-start", "step-finish", "snapshot", "patch", "tool-result", "tool_result"].includes(part.type)) {
    return true;
  }
  const prose = softText(part);
  return prose !== null && prose.trim() === "";
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
  const label = toolDisplayName(entries[0].part, type);
  const firstKey = entries[0].part.callID || entries[0].part.toolCallId || `tool-group-${type}`;

  if (entries.length < 2) {
    return renderTool(entries[0], firstKey, subagentMessages, onOpenSession);
  }

  const keyFor = (entry: ToolEntry, index: number) =>
    entry.part.callID || entry.part.toolCallId || `${firstKey}-${index}`;
  const statuses = entries.map((entry) => callStatus(entry.part));
  const failed = statuses.filter((status) => status === "error").length;
  const running = statuses.filter((status) => status !== "error" && status !== "completed").length;
  const total = groupDuration(entries);

  // These calls ran one after another at the same level — none of them owns the
  // others. Summarising the run as a whole keeps them peers; promoting the
  // first and folding the rest beneath it invented a parent that never existed.
  return (
    <div className={`tool-run${expanded ? " is-expanded" : ""}`}>
      <button
        type="button"
        className="tool-run-summary"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        <Layers3 size={13} className="tool-run-icon" />
        <span className="tool-run-count">{entries.length}</span>
        <span className="tool-run-label">{label} calls</span>
        <span className="tool-run-ticks" aria-hidden="true">
          {statuses.map((status, index) => (
            <span key={keyFor(entries[index], index)} className={`tool-run-tick is-${status}`} />
          ))}
        </span>
        <span className="tool-run-meta">
          {failed > 0 && <span className="tool-run-failed">{failed} failed</span>}
          {running > 0 && <span className="tool-run-running">{running} running</span>}
          {total != null && <span className="tool-run-time">{formatDuration(total)}</span>}
        </span>
        <ChevronDown size={13} className="tool-run-chevron" />
      </button>
      {expanded && (
        <div className="tool-run-items">
          {entries.map((entry, index) => renderTool(
            entry,
            keyFor(entry, index),
            subagentMessages,
            onOpenSession,
          ))}
        </div>
      )}
    </div>
  );
}

/** Normalised status for one call: "completed" | "error" | "running". */
function callStatus(part: MessagePart): string {
  const status = part.state?.status;
  if (status === "completed" || status === "error") return status;
  return "running";
}

/** Total wall time across a run, when every call reported both ends. */
function groupDuration(entries: ToolEntry[]): number | null {
  let total = 0;
  for (const { part } of entries) {
    const start = part.state?.time?.start;
    const end = part.state?.time?.end;
    if (typeof start !== "number" || typeof end !== "number") return null;
    total += end - start;
  }
  return total;
}

/**
 * Render parts in order while preserving text, reasoning, and tool boundaries.
 * Task calls still consume child sessions while the group is being built.
 */
export function renderInterleavedContent(
  rawParts: { part: MessagePart; msgIdx: number }[],
  childSessions: SessionInfo[],
  subagentMessages?: Map<string, Message[]>,
  onOpenSession?: (sessionId: string) => void,
) {
  const allParts = nestToolParts(rawParts);
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
    // Skipped up front so an empty block never reaches the branches below and
    // becomes a blank paragraph or an empty thinking accordion.
    if (isTransparentPart(part)) {
      index++;
      continue;
    }

    if (part.type === "text" && part.text) {
      currentTextChunks.push(part.text);
      index++;
      continue;
    }

    if (part.type === "reasoning" || part.type === "thinking" || part.type === "analysis") {
      const reasoning: string[] = [];
      while (index < allParts.length && (allParts[index].part.type === "reasoning" || allParts[index].part.type === "thinking" || allParts[index].part.type === "analysis")) {
        const raw = allParts[index].part as MessagePart & Record<string, unknown>;
        const value = raw.text || raw.reasoning || raw.thinking || raw.analysis;
        if (typeof value === "string") reasoning.push(value);
        index++;
      }
      flushText();
      const text = reasoning.join("\n\n");
      if (part.type === "reasoning") {
        elements.push(<ReasoningBlock key={`reasoning-${index}`} text={text} />);
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
