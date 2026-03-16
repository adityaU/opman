import React, { useCallback, useState, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import { Copy, Check, WrapText, Download } from "lucide-react";

import { LANG_EXTENSIONS } from "./types";

// Register only the most commonly used languages for PrismLight
import javascript from "react-syntax-highlighter/dist/esm/languages/prism/javascript";
import typescript from "react-syntax-highlighter/dist/esm/languages/prism/typescript";
import jsx from "react-syntax-highlighter/dist/esm/languages/prism/jsx";
import tsx from "react-syntax-highlighter/dist/esm/languages/prism/tsx";
import json from "react-syntax-highlighter/dist/esm/languages/prism/json";
import bash from "react-syntax-highlighter/dist/esm/languages/prism/bash";
import css from "react-syntax-highlighter/dist/esm/languages/prism/css";
import python from "react-syntax-highlighter/dist/esm/languages/prism/python";
import rust from "react-syntax-highlighter/dist/esm/languages/prism/rust";
import markdown from "react-syntax-highlighter/dist/esm/languages/prism/markdown";
import yaml from "react-syntax-highlighter/dist/esm/languages/prism/yaml";
import toml from "react-syntax-highlighter/dist/esm/languages/prism/toml";
import go from "react-syntax-highlighter/dist/esm/languages/prism/go";
import sql from "react-syntax-highlighter/dist/esm/languages/prism/sql";
import diff from "react-syntax-highlighter/dist/esm/languages/prism/diff";

SyntaxHighlighter.registerLanguage("javascript", javascript);
SyntaxHighlighter.registerLanguage("js", javascript);
SyntaxHighlighter.registerLanguage("typescript", typescript);
SyntaxHighlighter.registerLanguage("ts", typescript);
SyntaxHighlighter.registerLanguage("jsx", jsx);
SyntaxHighlighter.registerLanguage("tsx", tsx);
SyntaxHighlighter.registerLanguage("json", json);
SyntaxHighlighter.registerLanguage("bash", bash);
SyntaxHighlighter.registerLanguage("shell", bash);
SyntaxHighlighter.registerLanguage("sh", bash);
SyntaxHighlighter.registerLanguage("css", css);
SyntaxHighlighter.registerLanguage("python", python);
SyntaxHighlighter.registerLanguage("py", python);
SyntaxHighlighter.registerLanguage("rust", rust);
SyntaxHighlighter.registerLanguage("rs", rust);
SyntaxHighlighter.registerLanguage("markdown", markdown);
SyntaxHighlighter.registerLanguage("md", markdown);
SyntaxHighlighter.registerLanguage("yaml", yaml);
SyntaxHighlighter.registerLanguage("yml", yaml);
SyntaxHighlighter.registerLanguage("toml", toml);
SyntaxHighlighter.registerLanguage("go", go);
SyntaxHighlighter.registerLanguage("sql", sql);
SyntaxHighlighter.registerLanguage("diff", diff);

/** Stable codeTagProps — no allocation per render */
const CODE_TAG_PROPS = { style: { fontFamily: "var(--font-mono)" } };

/** Module-level stable remarkPlugins array — shared by all markdown renderers. */
export const REMARK_PLUGINS = [remarkGfm];

/** Interactive code block with line numbers, word wrap, copy, and download */
export const CodeBlock = React.memo(function CodeBlock({ language, code }: { language: string; code: string }) {
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

  // Memoize line numbers to avoid recalculating on wordWrap toggle
  const lineNumbers = useMemo(() => {
    const lineCount = code.split("\n").length;
    return Array.from({ length: lineCount }, (_, i) => i + 1);
  }, [code]);

  // Stable style object
  const codeStyle = useMemo(() => ({
    margin: 0,
    padding: 0,
    borderRadius: 0,
    background: "transparent",
    whiteSpace: wordWrap ? "pre-wrap" as const : "pre" as const,
    wordBreak: wordWrap ? "break-word" as const : "normal" as const,
    overflowX: wordWrap ? "hidden" as const : "auto" as const,
    flex: 1,
    minWidth: 0,
    fontFamily: "var(--font-mono)",
  }), [wordWrap]);

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
          codeTagProps={CODE_TAG_PROPS}
          customStyle={codeStyle}
        >
          {code}
        </SyntaxHighlighter>
      </div>
    </div>
  );
});

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
