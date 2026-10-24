import React, { useMemo, useState } from "react";
import { Check, Copy, FileOutput } from "lucide-react";
import { copyText } from "../terminal-panel/clipboard";
import { BashOutput } from "./BashOutput";
import { OutputView } from "./GenericToolViews";
import { readableOutput } from "./opmanTools";

/** Only known shell envelopes get a readable view; raw data is always available. */
function shellOutput(output: string): string | null {
  if (!output.trimStart().startsWith("{") || output.length > 200_000) return null;
  try {
    const value: unknown = JSON.parse(output);
    if (!value || typeof value !== "object" || Array.isArray(value)) return null;
    const data = value as Record<string, unknown>;
    if (typeof data.stdout !== "string" || typeof data.stderr !== "string") return null;
    if (data.exit_code !== null && typeof data.exit_code !== "number") return null;
    const omitted = ["stdout", "stderr"].flatMap(channel => {
      const bytes = data[`${channel}_omitted_bytes`];
      return typeof bytes === "number" && Number.isFinite(bytes) && bytes > 0
        ? [`[${channel} truncated: ${bytes} bytes omitted]`] : [];
    });
    return [readableOutput("bash", output), ...omitted].filter(Boolean).join("\n");
  } catch { return null; }
}

export function CodeModeResult({ output }: { output: string }) {
  const [raw, setRaw] = useState(false);
  const [copyResult, setCopyResult] = useState<{ text: string; ok: boolean } | null>(null);
  const readable = useMemo(() => shellOutput(output), [output]);
  const displayed = raw || readable === null ? output : readable;
  const copied = copyResult?.text === displayed && copyResult.ok;
  const copyFailed = copyResult?.text === displayed && !copyResult.ok;
  const copy = async () => setCopyResult({ text: displayed, ok: await copyText(displayed) });
  return <section className="code-mode-result" aria-label="Final code mode output">
    <div className="code-mode-result-head">
      <span className="code-mode-result-label"><FileOutput size={13} aria-hidden="true" />Final output</span>
      <div className="code-mode-actions">
        {readable !== null && <button type="button" aria-label="Show raw output"
          aria-pressed={raw} className="code-mode-action" onClick={() => { setRaw(value => !value); setCopyResult(null); }}>
          Raw
        </button>}
        {displayed && <button type="button" className="code-mode-action" onClick={copy}
          aria-label="Copy final output" title="Copy final output">
          {copied ? <Check size={12} /> : <Copy size={12} />}
          {copied ? "Copied" : "Copy"}
        </button>}
      </div>
    </div>
    <div className="code-mode-result-content">
      {!displayed ? <p className="code-mode-empty">(no output returned)</p>
        : raw ? <pre className="code-mode-raw">{output}</pre>
        : readable !== null ? <BashOutput output={readable} wrap />
        : <OutputView output={output} />}
    </div>
    {copyFailed && <p role="status" className="code-mode-copy-error">Could not copy. Select the output to copy it.</p>}
  </section>;
}
