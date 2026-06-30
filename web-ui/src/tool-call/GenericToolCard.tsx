import React, { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import { Wrench, ChevronDown, ChevronRight, AlertTriangle, Loader2 } from "lucide-react";
import type { MessagePart } from "../types";
import { formatToolName } from "./helpers";
import { asObj, TcStatus } from "./tcUtils";
import { markdownComponents, REMARK_PLUGINS } from "../message-turn/CodeBlock";

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
  const state = part.state;
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

  return (
    <div className={`gmc-card${isError ? " gmc-card-error" : ""}`}>
      <div className="gmc-card-head">
        <Wrench size={12} className="gmc-card-icon" />
        <span className="gmc-card-name">{shortName}</span>
        {state?.title && <span className="gmc-card-title">{state.title}</span>}
        <span className="gmc-card-status">
          <TcStatus status={status} durationMs={durationMs} />
        </span>
      </div>

      {(hasInput || hasOutput || isError || isRunning) && (
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

// ── InputView ─────────────────────────────────────────────────────

function InputView({ data }: { data: Record<string, unknown> | string }) {
  if (typeof data === "string") {
    return <pre className="gmc-pre">{data}</pre>;
  }
  const entries = Object.entries(data).filter(
    ([, v]) => v != null && v !== ""
  );
  if (entries.length === 0) return null;
  return (
    <div className="gmc-kv">
      {entries.map(([k, v]) => (
        <div key={k} className="gmc-kv-row">
          <span className="gmc-kv-key">{k}</span>
          <span className="gmc-kv-val">
            <KvValue value={v} />
          </span>
        </div>
      ))}
    </div>
  );
}

// ── KvValue ───────────────────────────────────────────────────────

function KvValue({ value }: { value: unknown }) {
  const [open, setOpen] = useState(false);

  if (value === null || value === undefined)
    return <span className="gmc-val-null">null</span>;
  if (typeof value === "boolean")
    return <span className="gmc-val-bool">{String(value)}</span>;
  if (typeof value === "number")
    return <span className="gmc-val-num">{value}</span>;
  if (typeof value === "string") {
    if (value.length > 160)
      return (
        <span className="gmc-val-str">
          {open ? value : value.slice(0, 160) + "…"}
          <button className="gmc-val-toggle" onClick={() => setOpen(!open)}>
            {open ? "less" : "more"}
          </button>
        </span>
      );
    return <span className="gmc-val-str">{value}</span>;
  }

  const label = Array.isArray(value)
    ? `Array[${(value as unknown[]).length}]`
    : `Object{${Object.keys(value as object).length}}`;
  return (
    <span className="gmc-val-obj">
      <button className="gmc-val-toggle" onClick={() => setOpen(!open)}>
        {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        {label}
      </button>
      {open && (
        <pre className="gmc-val-json">{JSON.stringify(value, null, 2)}</pre>
      )}
    </span>
  );
}

// ── OutputView ────────────────────────────────────────────────────

function OutputView({
  output,
  isLive,
}: {
  output: string;
  isLive?: boolean;
}) {
  const liveRef = React.useRef<HTMLPreElement>(null);
  React.useEffect(() => {
    if (isLive && liveRef.current)
      liveRef.current.scrollTop = liveRef.current.scrollHeight;
  }, [isLive, output]);

  const jsonData = useMemo(() => {
    const t = output.trim();
    if (!t.startsWith("{") && !t.startsWith("[")) return null;
    try {
      return JSON.parse(t);
    } catch {
      return null;
    }
  }, [output]);

  if (jsonData !== null) {
    if (
      typeof jsonData === "object" &&
      !Array.isArray(jsonData) &&
      jsonData !== null
    ) {
      const entries = Object.entries(jsonData as Record<string, unknown>);
      const isFlat =
        entries.length > 0 &&
        entries.length <= 24 &&
        entries.every(([, v]) => typeof v !== "object" || v === null);
      if (isFlat)
        return (
          <div className="gmc-kv">
            {entries.map(([k, v]) => (
              <div key={k} className="gmc-kv-row">
                <span className="gmc-kv-key">{k}</span>
                <span className="gmc-kv-val">
                  <KvValue value={v} />
                </span>
              </div>
            ))}
          </div>
        );
    }
    return (
      <SyntaxHighlighter
        useInlineStyles={false}
        language="json"
        PreTag="div"
        codeTagProps={CODE_TAG}
        customStyle={CODE_STYLE}
      >
        {JSON.stringify(jsonData, null, 2)}
      </SyntaxHighlighter>
    );
  }

  const trimmed = output.trim();
  const looksMarkdown =
    trimmed.startsWith("#") ||
    (trimmed.includes("**") && trimmed.includes("\n")) ||
    trimmed.startsWith("- ") ||
    trimmed.startsWith("* ");
  if (looksMarkdown)
    return (
      <div className="tool-output-markdown">
        <ReactMarkdown
          remarkPlugins={REMARK_PLUGINS}
          components={markdownComponents}
        >
          {output}
        </ReactMarkdown>
      </div>
    );

  return (
    <pre
      ref={liveRef}
      className={`gmc-pre${isLive ? " tool-call-live-output" : ""}`}
    >
      {output}
    </pre>
  );
}

