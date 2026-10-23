import React from "react";
import { createElement, type SyntaxHighlighterProps } from "react-syntax-highlighter";
import { SyntaxHighlighter } from "../utils/syntax";
import "../styles/code-source.css";

export interface CodeSourceProps {
  readonly language: string;
  readonly code: string;
  readonly wordWrap: boolean;
}

type RenderLines = NonNullable<SyntaxHighlighterProps["renderer"]>;
const CODE_PROPS = { className: "code-source-lines", style: { whiteSpace: "inherit" as const } };
const HIGHLIGHT_STYLE = { margin: 0, padding: 0, background: "transparent" };

/** The gutter and source share a row, so a wrapped line grows both together. */
const renderLines: RenderLines = ({ rows, stylesheet }) => (
  <span className="code-source-rows"
    style={{ "--code-source-digits": Math.max(2, String(rows.length).length) } as React.CSSProperties}>
    {rows.map((node, index) => (
      <span className="code-source-row" key={index} data-source-line={index + 1}>
        <span className="code-source-number" aria-hidden="true">{index + 1}</span>
        <span className="code-source-text">
          {createElement({ node, stylesheet, useInlineStyles: false, key: index })}
        </span>
      </span>
    ))}
  </span>
);

/** Wrap changes only CSS; the full snippet keeps its existing token tree. */
const HighlightedLines = React.memo(function HighlightedLines({ language, code }: Omit<CodeSourceProps, "wordWrap">) {
  return (
    <SyntaxHighlighter language={language} useInlineStyles={false}
      PreTag="div" className="code-source-highlight" codeTagProps={CODE_PROPS}
      customStyle={HIGHLIGHT_STYLE} wrapLines renderer={renderLines}>
      {code}
    </SyntaxHighlighter>
  );
});

export const CodeSource = React.memo(function CodeSource({ language, code, wordWrap }: CodeSourceProps) {
  return (
    <div className={`code-source ${wordWrap ? "code-source--wrap" : "code-source--nowrap"}`}>
      <HighlightedLines language={language} code={code} />
    </div>
  );
});
