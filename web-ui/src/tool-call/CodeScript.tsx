import React from "react";
import { SyntaxHighlighter } from "../utils/syntax";
import { InputView } from "./GenericToolViews";

const STYLE = { margin: 0, padding: 0, background: "transparent", fontSize: "inherit",
  fontFamily: "var(--font-mono)", maxHeight: 300, overflow: "auto" };

/** Codex wraps MCP arguments in {server, tool, arguments}; other runners send them directly. */
export function scriptArguments(input: Record<string, unknown> | string): Record<string, unknown> | string {
  if (typeof input === "string") {
    try {
      const parsed: unknown = JSON.parse(input);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return scriptArguments(parsed as Record<string, unknown>);
      }
    } catch { /* Freeform JavaScript is a normal tool input. */ }
    return input;
  }
  if (typeof input.server !== "string" || typeof input.tool !== "string") return input;
  const args = input.arguments;
  return typeof args === "string" ? scriptArguments(args)
    : args && typeof args === "object" && !Array.isArray(args) ? args as Record<string, unknown> : input;
}

export function CodeScript({ input }: { input: Record<string, unknown> | string }) {
  const args = scriptArguments(input);
  const code = typeof args === "string" ? args : args.code ?? args.script;
  if (typeof code !== "string") return <InputView data={args} />;
  return <SyntaxHighlighter language="javascript" useInlineStyles={false}
    PreTag="div" customStyle={STYLE}>{code}</SyntaxHighlighter>;
}
