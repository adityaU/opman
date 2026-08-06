import React, { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import { ChevronDown, ChevronRight } from "lucide-react";
import { markdownComponents, REMARK_PLUGINS } from "../message-turn/CodeBlock";

/** How much room an argument needs in the generic tool card. */
function isBlockValue(value: unknown): boolean {
  if (typeof value === "string") return value.length > 48 || value.includes("\n");
  return value !== null && typeof value === "object";
}

/** Arguments that read as code rather than prose. */
const CODE_KEYS = /^(command|cmd|script|code|query|sql|pattern|regex|path|file_path|url|snippet|content|diff)$/i;

export function InputView({ data }: { data: Record<string, unknown> | string }) {
  if (typeof data === "string") return <pre className="gmc-pre">{data}</pre>;
  const entries = Object.entries(data).filter(([, value]) => value != null && value !== "");
  if (entries.length === 0) return null;
  return (
    <dl className="gmc-args">
      {entries.map(([key, value]) => {
        const block = isBlockValue(value);
        return (
          <div key={key} className={`gmc-arg${block ? " is-block" : " is-inline"}${CODE_KEYS.test(key) ? " is-code" : ""}`}>
            <dt className="gmc-arg-key">{key}</dt>
            <dd className="gmc-arg-val"><KvValue value={value} /></dd>
          </div>
        );
      })}
    </dl>
  );
}

function KvValue({ value }: { value: unknown }) {
  const [open, setOpen] = useState(false);
  if (value === null || value === undefined) return <span className="gmc-val-null">null</span>;
  if (typeof value === "boolean") return <span className="gmc-val-bool">{String(value)}</span>;
  if (typeof value === "number") return <span className="gmc-val-num">{value}</span>;
  if (typeof value === "string") {
    if (value.length > 220 || value.split("\n").length > 6) {
      return (
        <span className={`gmc-val-str gmc-val-clamp${open ? " is-open" : ""}`}>
          <span className="gmc-val-text">{value}</span>
          <button type="button" className="gmc-val-toggle" onClick={() => setOpen(!open)} aria-expanded={open}>
            {open ? "Show less" : "Show all"}
          </button>
        </span>
      );
    }
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
      {open && <pre className="gmc-val-json">{JSON.stringify(value, null, 2)}</pre>}
    </span>
  );
}

export function OutputView({ output, isLive }: { output: string; isLive?: boolean }) {
  const liveRef = React.useRef<HTMLPreElement>(null);
  React.useEffect(() => {
    if (isLive && liveRef.current) liveRef.current.scrollTop = liveRef.current.scrollHeight;
  }, [isLive, output]);

  const jsonlRows = useMemo(() => {
    const lines = output.trim().split("\n").map((line) => line.trim()).filter((line) => line.startsWith("{") || line.startsWith("["));
    if (lines.length < 2) return null;
    const parsed: Record<string, unknown>[] = [];
    for (const line of lines) {
      try { parsed.push(JSON.parse(line)); } catch { return null; }
    }
    return parsed;
  }, [output]);

  const jsonData = useMemo(() => {
    if (jsonlRows !== null) return null;
    const text = output.trim();
    if (!text.startsWith("{") && !text.startsWith("[")) return null;
    try { return JSON.parse(text); } catch { return null; }
  }, [output, jsonlRows]);

  if (jsonlRows !== null) {
    return (
      <div className="gmc-jsonl">
        {jsonlRows.map((row, index) => (
          <div key={index} className="gmc-jsonl-row">
            {Object.entries(row).map(([key, value]) => (
              <span key={key} className="gmc-jsonl-field">
                <span className="gmc-jsonl-key">{key}</span>
                <span className="gmc-jsonl-val"><KvValue value={value} /></span>
              </span>
            ))}
          </div>
        ))}
      </div>
    );
  }

  if (jsonData !== null) {
    if (typeof jsonData === "object" && !Array.isArray(jsonData) && jsonData !== null) {
      const entries = Object.entries(jsonData as Record<string, unknown>);
      const isFlat = entries.length > 0 && entries.length <= 24 && entries.every(([, value]) => typeof value !== "object" || value === null);
      if (isFlat) {
        return (
          <div className="gmc-kv">
            {entries.map(([key, value]) => (
              <div key={key} className="gmc-kv-row">
                <span className="gmc-kv-key">{key}</span>
                <span className="gmc-kv-val"><KvValue value={value} /></span>
              </div>
            ))}
          </div>
        );
      }
    }
    return (
      <SyntaxHighlighter useInlineStyles={false} language="json" PreTag="div" codeTagProps={{ style: { fontFamily: "var(--font-mono)" } }} customStyle={{ margin: 0, fontSize: "0.7rem", maxHeight: 280, overflow: "auto", fontFamily: "var(--font-mono)" }}>
        {JSON.stringify(jsonData, null, 2)}
      </SyntaxHighlighter>
    );
  }

  const trimmed = output.trim();
  const looksMarkdown = trimmed.startsWith("#") || (trimmed.includes("**") && trimmed.includes("\n")) || trimmed.startsWith("- ") || trimmed.startsWith("* ");
  if (looksMarkdown) {
    return <div className="tool-output-markdown"><ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{output}</ReactMarkdown></div>;
  }
  return <pre ref={liveRef} className={`gmc-pre${isLive ? " tool-call-live-output" : ""}`}>{output}</pre>;
}
