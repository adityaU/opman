import React, { lazy, Suspense, useCallback, useId, useState } from "react";
import remarkGfm from "remark-gfm";
import { CodeSource } from "./CodeSource";
import { codePreviewKind } from "./codePreviewKind";
import "../styles/code-preview.css";
import { Copy, Check, WrapText, Download } from "lucide-react";

import { LANG_EXTENSIONS } from "./types";

const CodePreview = lazy(() => import("./CodePreview").then(module => ({ default: module.CodePreview })));

/** Module-level stable remarkPlugins array — shared by all markdown renderers. */
export const REMARK_PLUGINS = [remarkGfm];

/** Interactive code block with line numbers, word wrap, copy, and download */
export const CodeBlock = React.memo(function CodeBlock({ language, code }: { language: string; code: string }) {
  const [copied, setCopied] = useState(false);
  const [wordWrap, setWordWrap] = useState(true);
  const [view, setView] = useState<"render" | "code">("render");
  const preview = codePreviewKind(language);
  const rendered = preview !== null && view === "render";
  const panelId = useId();

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


  return (
    <div className={`code-block-wrapper ${wordWrap ? "" : "code-block-nowrap"}`}>
      <div className="code-block-header">
        <span>{language}</span>
        {preview && <div className="code-block-views" role="group" aria-label="Code block view">
          <button type="button" className="code-block-action-btn" aria-pressed={rendered}
            aria-controls={panelId} onClick={() => setView("render")}>Render</button>
          <button type="button" className="code-block-action-btn" aria-pressed={!rendered}
            aria-controls={panelId} onClick={() => setView("code")}>Code</button>
        </div>}
        <div className="code-block-actions">
          <button
            className={`code-block-action-btn ${wordWrap ? "active" : ""}`}
            onClick={() => setWordWrap((v) => !v)}
            aria-label="Toggle word wrap"
            aria-pressed={wordWrap}
            disabled={rendered}
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
      <div id={panelId} className={rendered ? "code-block-render" : "code-block-source"}>
        {rendered && preview
          ? <Suspense fallback={<p role="status">Rendering…</p>}><CodePreview kind={preview} code={code} /></Suspense>
          : <CodeSource language={language.toLowerCase()} code={code} wordWrap={wordWrap} />}
      </div>
    </div>
  );
});

/** Markdown renderer components (shared, no need to recreate per render). */
export const markdownComponents = {
  pre({ children }: { children?: React.ReactNode }) {
    if (React.isValidElement<{ className?: string }>(children)
      && children.props.className?.startsWith("language-")) return <>{children}</>;
    return <pre>{children}</pre>;
  },
  code(props: React.HTMLAttributes<HTMLElement> & { children?: React.ReactNode; node?: unknown }) {
    const { className, children, node: _node, ...rest } = props;
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
