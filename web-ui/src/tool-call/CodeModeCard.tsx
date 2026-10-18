import React, { useMemo, useState } from "react";
import "../styles/code-mode.css";
import { ChevronDown, ChevronRight, Code2 } from "lucide-react";
import type { MessagePart } from "../types";
import { useAutoOpen } from "../hooks/useAutoOpen";
import { InputView, OutputView } from "./GenericToolViews";
import { TcStatus } from "./tcUtils";
import { formatDuration } from "./helpers";
import { parseCodeResult, type CappedText, type InnerCall } from "./codeModeParse";
import { CodeReadPreview } from "./CodeReadPreview";

function CallText({ label, value }: { label: string; value: CappedText }) {
  return <div>
    <div className="gmc-section-label">{label}</div>
    <OutputView output={value.text || "(empty)"} />
    {value.truncated && <p className="tc-card-muted">{label} cut short ({value.bytes} bytes total).</p>}
  </div>;
}

function InnerTool({ call }: { call: InnerCall }) {
  const [opened, setOpened] = useState(false);
  const status = !call.finished ? "Stopped" : call.ok ? "Done" : "Error";
  return <details className="code-mode-call" onToggle={event => setOpened(event.currentTarget.open)}>
    <summary>
      <span className="code-mode-tool">{call.tool}</span>
      <span className={call.finished && !call.ok ? "tool-call-error-banner" : "tc-card-muted"}>
        {status} · {formatDuration(call.durationMs)}
      </span>
    </summary>
    <div className="code-mode-call-body">
      <CallText label="Input" value={call.input} />
      <CallText label={!call.finished || call.ok ? "Output" : "Error"} value={call.output} />
      {opened && <CodeReadPreview call={call} />}
    </div>
  </details>;
}

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
  return <div className={`gmc-card${status === "error" ? " gmc-card-error" : ""}`}>
    <button type="button" className="gmc-card-head gmc-card-head-btn"
      aria-expanded={expanded} onClick={() => setExpanded(value => !value)}>
      <span className="gmc-card-chevron">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
      <Code2 size={12} className="gmc-card-icon" />
      <span className="gmc-card-name">Execute</span>
      <span className="gmc-card-title">{result.calls.length} tool {result.calls.length === 1 ? "call" : "calls"}</span>
      <span className="gmc-card-status"><TcStatus status={status} durationMs={duration} /></span>
    </button>
    {expanded && <div className="gmc-card-body">
      {state?.input && <div><div className="gmc-section-label">Script</div><InputView data={state.input} /></div>}
      {result.output && <div><div className="gmc-section-label">Output</div><OutputView output={result.output} /></div>}
      {result.calls.length > 0 && <section aria-label="Internal tool calls">
        <div className="gmc-section-label">Internal tool calls</div>
        {result.calls.map((call, index) => <InnerTool key={index} call={call} />)}
      </section>}
      {state?.error && <div className="tool-call-error-banner">{state.error}</div>}
    </div>}
  </div>;
}
