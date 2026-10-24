import React, { useMemo, useState } from "react";
import { Check, ChevronRight, CircleStop, X } from "lucide-react";
import { OutputView } from "./GenericToolViews";
import { formatDuration } from "./helpers";
import { isCodeModeTool, parseCodeResult, type CappedText, type InnerCall } from "./codeModeParse";
import { CodeReadPreview, hasCodePreview } from "./CodeReadPreview";
import { BashOutput } from "./BashOutput";
import { canonicalToolLeaf, opmanDisplayName, readableOutput } from "./opmanTools";
import { CodeScript } from "./CodeScript";

function CallText({ label, value, shellCommand }: {
  label: string; value: CappedText; shellCommand?: string;
}) {
  return <div>
    <div className="gmc-section-label">{label}</div>
    {shellCommand !== undefined
      ? <BashOutput output={value.text || "(empty)"} command={shellCommand} wrap />
      : <OutputView output={value.text || "(empty)"} />}
    {value.truncated && <p className="tc-card-muted">{label} cut short ({value.bytes} bytes total).</p>}
  </div>;
}

function summarizeInput(inputText: string): string {
  try {
    const args = JSON.parse(inputText) as Record<string, unknown>;
    if (!args || typeof args !== "object" || Array.isArray(args)) return "";
    const target =
      args.command ??
      args.cmd ??
      args.pattern ??
      args.query ??
      args.path ??
      args.file_path ??
      args.filePath ??
      args.url;
    if (typeof target === "string" && target.trim()) {
      const clean = target.trim();
      return clean;
    }
  } catch { /* ignore */ }
  return "";
}

/** Only recorded tool results create child calls; source code never invents a call. */
export function CodeModeInnerTool({ call, depth = 0 }: { call: InnerCall; depth?: number }) {
  const [opened, setOpened] = useState(false);
  const leaf = useMemo(() => canonicalToolLeaf(call.tool), [call.tool]);
  const shellCommand = useMemo(() => {
    if (leaf !== "bash" && !/(?:^|[._])(?:bash|shell|run_command)$/i.test(call.tool)) return undefined;
    try {
      const args = JSON.parse(call.input.text);
      const command: unknown = args?.command ?? args?.cmd;
      return typeof command === "string" ? command : "";
    } catch { return ""; }
  }, [call.tool, call.input.text, leaf]);
  const result = useMemo(() => opened && depth < 16 ? parseCodeResult(call.output.text) : null,
    [opened, depth, call.output.text]);
  const output = useMemo(() => {
    let text = result?.output ?? call.output.text;
    if (call.ok && leaf && leaf !== "read_file") {
      text = readableOutput(leaf, text);
    }
    return { ...call.output, text };
  }, [call.ok, call.output, leaf, result]);
  const targetSummary = useMemo(() => summarizeInput(call.input.text), [call.input.text]);
  const richPreview = useMemo(() => call.ok && !call.output.truncated && hasCodePreview(call), [call]);
  const status = !call.finished ? "Stopped" : call.ok ? "Done" : "Error";
  const label = opmanDisplayName(call.tool.replace(/^opman[-_]tools\./i, "opman_tools_")) ?? call.tool;
  return <details className="code-mode-call" onToggle={event => setOpened(event.currentTarget.open)}>
    <summary>
      <span className="code-mode-call-name"><ChevronRight size={12} className="code-mode-call-chevron" aria-hidden="true" />
        <span className="code-mode-tool">{label}</span>
      </span>
      {targetSummary && <span className="code-mode-call-target" title={targetSummary}>{targetSummary}</span>}
      <span className={`code-mode-call-meta${call.finished && !call.ok ? " is-error" : ""}`}>
        {!call.finished ? <CircleStop size={12} aria-hidden="true" />
          : call.ok ? <Check size={12} className="code-mode-call-success" aria-hidden="true" />
          : <X size={12} aria-hidden="true" />}
        {status} · {formatDuration(call.durationMs)}
      </span>
    </summary>
    {opened && <div className="code-mode-call-body">
      {isCodeModeTool(call.tool)
        ? <div><div className="gmc-section-label">Script</div><CodeScript input={call.input.text} wrap /></div>
        : <details className="code-mode-arguments"><summary><ChevronRight size={10} aria-hidden="true" />Arguments</summary>
          <CallText label="Input" value={call.input} />
        </details>}
      {Boolean(result?.calls.length) && <section className="code-mode-children" aria-label="Internal tool calls">
        {result?.calls.map((child, index) => <CodeModeInnerTool key={index} call={child} depth={depth + 1} />)}
      </section>}
      {!richPreview && (
        <CallText label={!call.finished || call.ok ? "Output" : "Error"} value={output} shellCommand={call.ok ? shellCommand : undefined} />
      )}
      <CodeReadPreview call={call} />
      {(output.text !== call.output.text || richPreview) && <details className="code-mode-arguments">
        <summary><ChevronRight size={10} aria-hidden="true" />Raw result</summary><pre className="code-mode-raw">{call.output.text}</pre>
      </details>}
    </div>}
  </details>;
}
