import React, { useMemo, useState } from "react";
import "../styles/code-mode.css";
import { ChevronDown, ChevronRight, Code2 } from "lucide-react";
import type { MessagePart } from "../types";
import { useAutoOpen } from "../hooks/useAutoOpen";
import { OutputView } from "./GenericToolViews";
import { TcStatus } from "./tcUtils";
import { isNodeReplTool, parseCodeResult } from "./codeModeParse";
import { CodeModeInnerTool } from "./CodeModeInnerTool";
import { CodeScript } from "./CodeScript";
import { NestedToolCalls } from "./NestedToolCalls";

export function CodeModeCard({ part }: { part: MessagePart }) {
  const name = part.tool || part.toolName || "code_execute";
  const { shouldAutoOpen } = useAutoOpen();
  const [expanded, setExpanded] = useState(() => shouldAutoOpen(name));
  const state = part.state;
  const raw = state?.output || state?.metadata?.output || "";
  const result = useMemo(() => parseCodeResult(raw, state?.metadata?.structuredContent),
    [raw, state?.metadata?.structuredContent]);
  const status = state?.status || "pending";
  const duration = state?.time?.start != null && state?.time?.end != null
    ? state.time.end - state.time.start : null;
  const count = result.calls.length + (part.toolChildren?.length ?? 0);
  const showOutput = Boolean(
    result.output &&
      !(count > 0 && result.output === "(the script returned nothing)") &&
      !(result.calls.length === 1 && result.output === result.calls[0].output.text)
  );
  return <div className={`gmc-card${status === "error" ? " gmc-card-error" : ""}`}>
    <button type="button" className="gmc-card-head gmc-card-head-btn"
      aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
      <span className="gmc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
      <Code2 size={12} className="gmc-card-icon" />
      <span className="gmc-card-name">{isNodeReplTool(name) ? "JavaScript" : "Execute"}</span>
      <span className="gmc-card-title">{count > 0 ? `${count} tool ${count === 1 ? "call" : "calls"}` : "Code mode"}</span>
      <span className="gmc-card-status"><TcStatus status={status} durationMs={duration} /></span>
    </button>
    {expanded && <div className="gmc-card-body">
      {state?.input && <div><div className="gmc-section-label">Script</div><CodeScript input={state.input} /></div>}
      {showOutput && <div><div className="gmc-section-label">Output</div><OutputView output={result.output} /></div>}
      {result.calls.length > 0 && <section aria-label="Internal tool calls">
        <div className="gmc-section-label">Internal tool calls</div>
        {result.calls.map((call, index) => <CodeModeInnerTool key={index} call={call} />)}
      </section>}
      <NestedToolCalls parts={part.toolChildren} />
      {state?.error && <div className="tool-call-error-banner">{state.error}</div>}
    </div>}
  </div>;
}
