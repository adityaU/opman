import React from "react";
import { createElement, type SyntaxHighlighterProps } from "react-syntax-highlighter";
import { SyntaxHighlighter } from "../utils/syntax";
import type { DiffRow } from "./diffRows";

type Render = NonNullable<SyntaxHighlighterProps["renderer"]>;
const STYLE = { margin: 0, padding: 0, background: "transparent", fontFamily: "inherit" };

/** Tokenize both versions as complete snippets so multiline tokens keep their state. */
export const DiffHunk = React.memo(function DiffHunk({
  oldStr, newStr, rows: diff, language,
}: { oldStr: string; newStr: string; rows: DiffRow[]; language: string }) {
  const renderOld: Render = ({ rows: oldRows, stylesheet }) => {
    const renderNew: Render = ({ rows: newRows }) => {
      let oldIndex = 0;
      let newIndex = 0;
      return diff.map((row, index) => {
        const node = row.type === "del" ? oldRows[oldIndex] : newRows[newIndex];
        if (row.type !== "add") oldIndex++;
        if (row.type !== "del") newIndex++;
        const kind = row.type === "add" ? " added" : row.type === "del" ? " removed" : " context";
        const sign = row.type === "add" ? "+" : row.type === "del" ? "-" : " ";
        return <div key={index} className={`diff-line${kind}`}>
          <span className="tool-diff-marker" aria-hidden="true">{sign} </span>
          <span className="diff-line-content">{node
            ? createElement({ node, stylesheet, useInlineStyles: false, key: index })
            : row.text}</span>
        </div>;
      });
    };
    return <SyntaxHighlighter language={language} useInlineStyles={false} wrapLines
      PreTag="div" CodeTag="div" customStyle={STYLE} renderer={renderNew}>
      {newStr}
    </SyntaxHighlighter>;
  };
  return <SyntaxHighlighter language={language} useInlineStyles={false} wrapLines
    PreTag="div" CodeTag="div" customStyle={STYLE} renderer={renderOld}>
    {oldStr}
  </SyntaxHighlighter>;
});
