import React, { useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock3,
  Loader2,
  MessageCircle,
} from "lucide-react";
import type { MessagePart } from "../../types";
import { asArr, asObj, str, TcStatus } from "../tcUtils";
import { useAutoOpen } from "../../hooks/useAutoOpen";
import { ACTION_META, agentManagerAction } from "./action";
import { AgentTranscript, MetaChip, OpenSessionLink, Summary, TargetChip } from "./atoms";
import { AbortBody, ListBody, OptionsBody, StartBody, WaitBody } from "./bodies";
import { asText, deliveryLabel, openableSessionId, shortId } from "./model";

/**
 * One card for every agent-manager MCP tool.
 *
 * The head is identical across actions — chevron, icon, label, gist, status —
 * so it lives here; everything below it comes from `bodies`. What the head
 * shows collapsed is the one thing the reader wants without expanding, which is
 * different per action and so lives in `Summary`.
 */
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
  const output = useMemo(() => asObj(state?.output), [state?.output]);
  const rawOutput = typeof state?.output === "string" ? state.output : "";
  const [expanded, setExpanded] = useState(() => isRunning || shouldAutoOpen(toolName));
  const durationMs =
    state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  const message = str(input.message);
  const messages = asArr(output.messages);
  const queued = asText(output.queued_messages);
  const busy = output.busy === true ? "busy" : output.busy === false ? "idle" : "";
  const runner = str(output.runner) || str(input.runner);
  const model = str(input.model);
  const sessionId = str(output.session_id);
  const physicalId = str(output.physical_session_id);
  const openable = openableSessionId(action, input, output);
  const delivery = deliveryLabel(str(output.delivery) || str(input.delivery));
  const startTitle = str(input.title) || shortId(sessionId) || "Agent session";
  const showRawOutput = !message && messages.length === 0 && rawOutput && !hasFields(output);

  return (
    <div className={`am-card am-card-${meta.tone}${isError ? " am-card-error" : ""}`}>
      <button
        className="am-card-head am-card-head-btn"
        onClick={() => setExpanded((value) => !value)}
        type="button"
        aria-expanded={expanded}
      >
        <span className="am-card-chevron">
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
        <span className="am-card-icon">
          <meta.Icon size={13} />
        </span>
        <span className="am-card-label">{meta.label}</span>
        <Summary action={action} input={input} output={output} />
        <span className="am-card-status">
          <TcStatus status={status} durationMs={durationMs} />
        </span>
      </button>

      {expanded && (
        <div className="am-card-body">
          {action === "start" && (
            <StartBody sessionId={sessionId} title={startTitle} running={isRunning} />
          )}

          {message && (
            <div className="am-message-block">
              <div className="am-section-label">
                <MessageCircle size={11} /> Message
              </div>
              <div className="am-message">{message}</div>
            </div>
          )}

          {action === "list" && !isRunning && <ListBody output={output} />}
          {action === "wait" && !isRunning && (
            <WaitBody output={output} sessionId={openable} timeout={asText(input.timeout)} />
          )}
          {action === "abort" && !isRunning && (
            <AbortBody output={output} sessionId={openable} />
          )}
          {action === "options" && !isRunning && <OptionsBody output={output} />}

          {action !== "list" && action !== "options" && (
            <div className="am-meta-row">
              {action === "send" && <TargetChip input={input} />}
              {runner && <MetaChip label="runner" value={runner} />}
              {model && <MetaChip label="model" value={model} />}
              {NAMES_DELIVERY.has(action) && delivery && (
                <MetaChip label="delivery" value={delivery} icon={<Clock3 size={10} />} />
              )}
              {busy && <MetaChip label="state" value={busy} icon={<Activity size={10} />} />}
              {queued && <MetaChip label="queued" value={queued} />}
              {action !== "start" && action !== "wait" && action !== "abort" && (
                <OpenSessionLink sessionId={openable} label={shortId(openable) || "agent"} />
              )}
            </div>
          )}

          {(sessionId || physicalId) && (
            <div className="am-id-row">
              {sessionId && <MetaChip label="agent" value={sessionId} />}
              {physicalId && physicalId !== sessionId && (
                <MetaChip label="runner session" value={physicalId} />
              )}
            </div>
          )}

          <AgentTranscript messages={messages} />

          {showRawOutput && <pre className="am-card-pre">{rawOutput}</pre>}

          {isRunning && !rawOutput && (
            <div className="am-card-muted">
              <Loader2 size={11} className="tool-spin-icon" /> Waiting for manager response…
            </div>
          )}

          {isError && (
            <div className="am-card-error-message">
              <AlertTriangle size={12} />
              <span>{state?.error || rawOutput || "Agent manager request failed"}</span>
            </div>
          )}

          {!isRunning && !isError && action === "progress" && messages.length === 0 && (
            <div className="am-card-muted">
              <CheckCircle2 size={11} /> No recent transcript messages
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/** Only the two tools that dispatch a turn have a delivery to report. */
const NAMES_DELIVERY: ReadonlySet<string> = new Set(["start", "send"]);

const hasFields = (output: Record<string, unknown>): boolean => Object.keys(output).length > 0;
