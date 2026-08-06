import React, { useState } from "react";
import { Wrench, ChevronDown, ChevronRight, AlertTriangle, Loader2, CheckCircle2, XCircle } from "lucide-react";
import type { MessagePart } from "../types";
import { formatToolName } from "./helpers";
import { TcStatus } from "./tcUtils";
import { InputView, OutputView } from "./GenericToolViews";
import { useAutoOpen } from "../hooks/useAutoOpen";

// ── Generic MCP Tool Card ─────────────────────────────────────────
// Catches any tool without a dedicated card. Shows input as KV pairs,
// output as structured JSON / rendered markdown / plain text.

const CODE_TAG = { style: { fontFamily: "var(--font-mono)" } };
const CODE_STYLE = {
  margin: 0,
  fontSize: "0.7rem",
  maxHeight: 280,
  overflow: "auto" as const,
  fontFamily: "var(--font-mono)",
};

export function GenericToolCard({ part }: { part: MessagePart }) {
  const toolName = part.tool || part.toolName || "unknown";
  const shortName = formatToolName(toolName);
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(toolName));

  const state = part.state;
  const progressTitle = state?.title;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isRunning = status === "running" || status === "pending";
  const durationMs =
    state?.time?.start && state?.time?.end ? state.time.end - state.time.start : null;

  const inputData = state?.input;
  const hasInput =
    inputData != null &&
    (typeof inputData === "string"
      ? inputData.length > 0
      : Object.keys(inputData).length > 0);

  const finalOutput = state?.output;
  const liveOutput =
    typeof state?.metadata?.output === "string" ? state.metadata.output : null;
  const outputRaw =
    finalOutput && finalOutput.length > 0 ? finalOutput : liveOutput;
  const hasOutput = outputRaw != null && outputRaw.length > 0;

  const toggle = () => setExpanded(e => !e);

  return (
    <div className={`gmc-card${isError ? " gmc-card-error" : ""}`}>
      <button className="gmc-card-head gmc-card-head-btn" onClick={toggle}>
        <span className="gmc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
        <Wrench size={12} className="gmc-card-icon" />
        <span className="gmc-card-name">{shortName}</span>
        <span className="gmc-card-status">
          <TcStatus status={status} durationMs={durationMs} />
        </span>
      </button>

      {progressTitle && (
        <div className="gmc-progress" role="status" aria-live="polite">
          {isRunning ? (
            <Loader2 size={12} className="tool-spin-icon" />
          ) : isError ? (
            <XCircle size={12} className="tool-error-icon" />
          ) : (
            <CheckCircle2 size={12} className="tool-success-icon" />
          )}
          <span className="gmc-progress-label">Progress</span>
          <span className="gmc-progress-text">{progressTitle}</span>
        </div>
      )}

      {expanded && (hasInput || hasOutput || isError || isRunning) && (
        <div className="gmc-card-body">
          {hasInput && (
            <div>
              <div className="gmc-section-label">Input</div>
              <InputView data={inputData!} />
            </div>
          )}

          {hasOutput && (
            <div>
              <div className="gmc-section-label">Output</div>
              <OutputView
                output={outputRaw!}
                isLive={isRunning && !finalOutput?.length}
              />
            </div>
          )}

          {!hasOutput && isRunning && (
            <div className="tc-card-muted">
              <Loader2 size={11} className="tool-spin-icon" /> Running…
            </div>
          )}

          {isError && (
            <div className="tool-call-error-banner">
              <AlertTriangle size={12} />
              <span>{state?.error || "Tool call failed"}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
