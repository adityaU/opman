import React, { useEffect, useMemo, useRef } from "react";
import { SyntaxHighlighter } from "../utils/syntax";
import { ansiChunks } from "./ansiOutput";
import { bashOutputLanguage, stripTerminalControls } from "./bashOutputFormat";
import { CodeBlock } from "../message-turn/CodeBlock";
import { codePreviewKind } from "../message-turn/codePreviewKind";

const STYLE = { margin: 0, padding: 0, background: "transparent", fontFamily: "var(--font-mono)",
  fontSize: "inherit", whiteSpace: "pre-wrap" as const, overflowWrap: "anywhere" as const };

export const BashOutput = React.memo(function BashOutput({
  output, command = "", isLive = false, wrap = false,
}: { output: string; command?: string; isLive?: boolean; wrap?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const formatted = useMemo(() => {
    const text = stripTerminalControls(output);
    const language = bashOutputLanguage(text, command);
    // Coloured diffs still use background-only changes.
    const chunks = language !== "diff" && output.includes("\x1b[") ? ansiChunks(output) : null;
    return { text, language, chunks };
  }, [output, command]);
  useEffect(() => {
    if (isLive && ref.current) ref.current.scrollTop = ref.current.scrollHeight;
  }, [output, isLive]);
  if (!formatted.chunks && codePreviewKind(formatted.language) && formatted.text.length <= 200_000) {
    return <CodeBlock language={formatted.language} code={formatted.text} />;
  }
  return <div ref={ref} className={`tool-call-pre bash-output${isLive ? " tool-call-live-output" : ""}`}>
    {formatted.language === "diff"
      ? <pre style={STYLE}>{(formatted.text.match(/[^\n]*\n|[^\n]+$/g) ?? []).map((line, index) => {
          const kind = line.startsWith("+") && !line.startsWith("+++ ") ? " added"
            : line.startsWith("-") && !line.startsWith("--- ") ? " removed" : "";
          return <span key={index} className={`bash-output-diff-line${kind}`}>{line}</span>;
        })}</pre>
      : formatted.chunks
      ? <pre style={STYLE}>{formatted.chunks.map((chunk, index) =>
          <span key={index} style={chunk.style}>{chunk.text}</span>)}</pre>
      : formatted.text.length > 200_000
        ? <pre style={STYLE}>{formatted.text}</pre>
        : <SyntaxHighlighter language={formatted.language} useInlineStyles={false}
            PreTag="div" customStyle={STYLE}
            codeTagProps={wrap ? { style: { whiteSpace: "pre-wrap", overflowWrap: "anywhere" } } : undefined}>{formatted.text}</SyntaxHighlighter>}
  </div>;
});
