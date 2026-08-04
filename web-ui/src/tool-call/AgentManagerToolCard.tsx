import React, { useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock3,
  Loader2,
  MessageCircle,
  Play,
  Send,
} from "lucide-react";
import type { MessagePart } from "../types";
import { asObj, str, TcStatus } from "./tcUtils";
import { useAutoOpen } from "../hooks/useAutoOpen";

export type AgentManagerAction = "start" | "send" | "progress" | "other";

/** Match names from any MCP host prefix, for example mcp__agent-manager__agent_send. */
export function agentManagerAction(toolName: string): AgentManagerAction {
  const name = toolName.toLowerCase();
  if (name.includes("agent_start")) return "start";
  if (name.includes("agent_send")) return "send";
  if (name.includes("agent_progress")) return "progress";
  return "other";
}

export function isAgentManagerTool(toolName: string): boolean {
  return agentManagerAction(toolName) !== "other";
}

export function parseAgentOutput(output: unknown): Record<string, unknown> {
  return asObj(output);
}

const ACTION_META: Record<AgentManagerAction, {
  label: string;
  tone: string;
  Icon: typeof Bot;
}> = {
  start: { label: "Start agent", tone: "start", Icon: Play },
  send: { label: "Send to agent", tone: "send", Icon: Send },
  progress: { label: "Agent progress", tone: "progress", Icon: Activity },
  other: { label: "Agent manager", tone: "progress", Icon: Bot },
};

function shortId(value: string): string {
  if (!value) return "";
  return value.length > 18 ? `${value.slice(0, 10)}…${value.slice(-5)}` : value;
}

function asText(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

function messageText(message: unknown): string {
  const value = asObj(message);
  const direct = str(value.text);
  if (direct) return direct;
  const parts = Array.isArray(value.parts) ? value.parts : [];
  return parts
    .map((part) => asText(asObj(part).text))
    .filter(Boolean)
    .join("\n");
}

function messageRole(message: unknown): string {
  const role = str(asObj(asObj(message).info).role);
  return role === "assistant" || role === "user" ? role : "system";
}

function displayTarget(input: Record<string, unknown>, output: Record<string, unknown>): string {
  return str(input.to) || (str(input.agent_id) ? str(input.agent_id) : "parent");
}

function deliveryLabel(value: string): string {
  return value === "queued" || value === "next_turn" || value === "next-turn"
    ? "next turn"
    : value === "none"
      ? "created"
      : "immediate";
}

function Summary({
  action,
  input,
  output,
}: {
  action: AgentManagerAction;
  input: Record<string, unknown>;
  output: Record<string, unknown>;
}) {
  if (action === "send") {
    return (
      <span className="am-card-summary">
        {displayTarget(input, output)}
        <span className={`am-delivery am-delivery-${deliveryLabel(str(output.delivery) || str(input.delivery)).replace(" ", "-")}`}>
          {deliveryLabel(str(output.delivery) || str(input.delivery))}
        </span>
      </span>
    );
  }
  if (action === "progress") {
    return <span className="am-card-summary">{displayTarget(input, output)}</span>;
  }
  const runner = str(output.runner) || str(input.runner);
  const model = str(input.model);
  return (
    <span className="am-card-summary">
      {runner || "default runner"}
      {model && <span className="am-card-model">{model}</span>}
    </span>
  );
}

function MetaChip({ label, value, icon }: { label?: string; value: string; icon?: React.ReactNode }) {
  if (!value) return null;
  return (
    <span className="am-meta-chip" title={value}>
      {icon}
      {label && <span className="am-meta-label">{label}</span>}
      <span>{value}</span>
    </span>
  );
}

function AgentTranscript({ messages }: { messages: unknown[] }) {
  if (messages.length === 0) return null;
  return (
    <div className="am-transcript">
      {messages.slice(0, 8).map((message, index) => {
        const text = messageText(message);
        if (!text) return null;
        const role = messageRole(message);
        return (
          <div className={`am-transcript-row am-transcript-${role}`} key={`${role}-${index}`}>
            <span className="am-transcript-role">{role}</span>
            <span className="am-transcript-text">{text}</span>
          </div>
        );
      })}
    </div>
  );
}

export function AgentManagerToolCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "";
  const action = agentManagerAction(toolName);
  const meta = ACTION_META[action];
  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const { shouldAutoOpen } = useAutoOpen();
  const input = useMemo(() => asObj(state?.input ?? part.args), [state?.input, part.args]);
  const output = useMemo(() => parseAgentOutput(state?.output), [state?.output]);
  const rawOutput = typeof state?.output === "string" ? state.output : "";
  const [expanded, setExpanded] = useState(() => isRunning || shouldAutoOpen(toolName));
  const durationMs = state?.time?.start && state?.time?.end
    ? state.time.end - state.time.start
    : null;
  const message = str(input.message);
  const messages = Array.isArray(output.messages) ? output.messages : [];
  const queued = asText(output.queued_messages);
  const busy = output.busy === true ? "busy" : output.busy === false ? "idle" : "";
  const runner = str(output.runner) || str(input.runner);
  const model = str(input.model);
  const sessionId = str(output.session_id);
  const physicalId = str(output.physical_session_id);
  const delivery = deliveryLabel(str(output.delivery) || str(input.delivery));
  const summaryOutput = output;

  return (
    <div className={`am-card am-card-${meta.tone}${isError ? " am-card-error" : ""}`}>
      <button
        className="am-card-head am-card-head-btn"
        onClick={() => setExpanded((value) => !value)}
        type="button"
        aria-expanded={expanded}
      >
        <span className="am-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <span className="am-card-icon"><meta.Icon size={13} /></span>
        <span className="am-card-label">{meta.label}</span>
        <Summary action={action} input={input} output={summaryOutput} />
        <span className="am-card-status"><TcStatus status={status} durationMs={durationMs} /></span>
      </button>

      {expanded && (
        <div className="am-card-body">
          {action === "start" && (
            <div className="am-start-banner">
              <Bot size={15} />
              <span>{sessionId ? `Agent ${shortId(sessionId)} is ready` : isRunning ? "Creating an agent session…" : "Agent session"}</span>
            </div>
          )}

          {message && (
            <div className="am-message-block">
              <div className="am-section-label"><MessageCircle size={11} /> Message</div>
              <div className="am-message">{message}</div>
            </div>
          )}

          <div className="am-meta-row">
            {action === "send" && <MetaChip value={displayTarget(input, output)} icon={<Send size={10} />} />}
            {runner && <MetaChip label="runner" value={runner} />}
            {model && <MetaChip label="model" value={model} />}
            {action !== "progress" && delivery && <MetaChip label="delivery" value={delivery} icon={<Clock3 size={10} />} />}
            {busy && <MetaChip label="state" value={busy} icon={<Activity size={10} />} />}
            {queued && <MetaChip label="queued" value={queued} />}
          </div>

          {(sessionId || physicalId) && (
            <div className="am-id-row">
              {sessionId && <MetaChip label="agent" value={sessionId} />}
              {physicalId && physicalId !== sessionId && <MetaChip label="runner session" value={physicalId} />}
            </div>
          )}

          <AgentTranscript messages={messages} />

          {!message && messages.length === 0 && rawOutput && Object.keys(output).length === 0 && (
            <pre className="am-card-pre">{rawOutput}</pre>
          )}

          {isRunning && !rawOutput && (
            <div className="am-card-muted"><Loader2 size={11} className="tool-spin-icon" /> Waiting for manager response…</div>
          )}

          {isError && (
            <div className="am-card-error-message">
              <AlertTriangle size={12} />
              <span>{state?.error || rawOutput || "Agent manager request failed"}</span>
            </div>
          )}

          {!isRunning && !isError && action === "progress" && messages.length === 0 && (
            <div className="am-card-muted"><CheckCircle2 size={11} /> No recent transcript messages</div>
          )}
        </div>
      )}
    </div>
  );
}
