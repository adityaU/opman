import React, { useCallback, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { Copy, Check, WrapText, Download } from "lucide-react";

import { LANG_EXTENSIONS } from "./types";

/** Interactive code block with line numbers, word wrap, copy, and download */
export function CodeBlock({ language, code }: { language: string; code: string }) {
  const [copied, setCopied] = useState(false);
  const [wordWrap, setWordWrap] = useState(true);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [code]);

  const handleDownload = useCallback(() => {
    const ext = LANG_EXTENSIONS[language] || "txt";
    const blob = new Blob([code], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `snippet.${ext}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [code, language]);

  // Generate line numbers
  const lineCount = code.split("\n").length;
  const lineNumbers = Array.from({ length: lineCount }, (_, i) => i + 1);

  return (
    <div className={`code-block-wrapper ${wordWrap ? "" : "code-block-nowrap"}`}>
      <div className="code-block-header">
        <span>{language}</span>
        <div className="code-block-actions">
          <button
            className={`code-block-action-btn ${wordWrap ? "active" : ""}`}
            onClick={() => setWordWrap((v) => !v)}
            aria-label="Toggle word wrap"
            title="Toggle word wrap"
          >
            <WrapText size={12} />
          </button>
          <button
            className="code-block-action-btn"
            onClick={handleDownload}
            aria-label="Download code"
            title="Download"
          >
            <Download size={12} />
          </button>
          <button
            className="code-block-action-btn"
            onClick={handleCopy}
            aria-label="Copy code"
            title="Copy"
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
          </button>
        </div>
      </div>
      <div className="code-block-body">
        <div className="code-block-line-numbers" aria-hidden="true">
          {lineNumbers.map((n) => (
            <span key={n}>{n}</span>
          ))}
        </div>
        <SyntaxHighlighter
          useInlineStyles={false}
          language={language}
          PreTag="div"
          customStyle={{
            margin: 0,
            padding: 0,
            borderRadius: 0,
            background: "transparent",
            whiteSpace: wordWrap ? "pre-wrap" : "pre",
            wordBreak: wordWrap ? "break-word" : "normal",
            overflowX: wordWrap ? "hidden" : "auto",
            flex: 1,
            minWidth: 0,
            fontFamily: "var(--font-mono)",
          }}
        >
          {code}
        </SyntaxHighlighter>
      </div>
    </div>
  );
}

/** Markdown renderer components (shared, no need to recreate per render). */
export const markdownComponents = {
  code(props: React.HTMLAttributes<HTMLElement> & { children?: React.ReactNode }) {
    const { className, children, ...rest } = props;
    const match = /language-(\w+)/.exec(className || "");
    const codeStr = String(children).replace(/\n$/, "");
    if (match) {
      return <CodeBlock language={match[1]} code={codeStr} />;
    }
    return (
      <code className="inline-code" {...rest}>
        {children}
      </code>
    );
  },
};
