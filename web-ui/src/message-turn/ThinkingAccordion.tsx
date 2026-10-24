import React from "react";
import { Brain, ChevronDown } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { markdownComponents, REMARK_PLUGINS } from "./CodeBlock";
import { useThinkingVisibility } from "../utils/thinkingVisibility";

type Props = { text: string };

export function ThinkingAccordion({ text }: Props) {
  const showThinking = useThinkingVisibility();
  if (!showThinking) return null;
  if (!text.trim()) return null;
  return (
    <details className="thinking-accordion">
      <summary><Brain size={14} /><span>Thinking</span><ChevronDown size={14} className="thinking-chevron" /></summary>
      <div className="thinking-content">
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>{text}</ReactMarkdown>
      </div>
    </details>
  );
}
