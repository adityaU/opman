import React from "react";
import type { MessagePart } from "../types";
import { ToolCall } from "./ToolCall";

export function NestedToolCalls({ parts }: { parts?: readonly MessagePart[] }) {
  if (!parts?.length) return null;
  return <section className="code-mode-children" aria-label="Internal tool calls">
    {parts.map((part, index) => <ToolCall key={part.callID ?? part.toolCallId ?? part.id ?? index} part={part} />)}
  </section>;
}
