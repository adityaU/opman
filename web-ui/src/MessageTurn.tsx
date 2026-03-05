import React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { Message, MessagePart, ModelRef } from "./types";
import { ToolCall } from "./ToolCall";
import { User, Bot, Wrench } from "lucide-react";

interface Props {
  message: Message;
  /** When true, this message has the same role as the previous one —
   *  hide the avatar and role label to visually group them. */
  isContinuation?: boolean;
}

export function MessageTurn({ message, isContinuation }: Props) {
  const { info, parts } = message;
  const role = info.role;

  if (role === "system") return null; // Hide system messages

  const isUser = role === "user";
  const isAssistant = role === "assistant";

  /** Render a model reference as a display string. */
  function modelLabel(m: ModelRef): string {
    if (typeof m === "string") return m;
    return m.modelID || JSON.stringify(m);
  }

  // Separate parts by type
  const textParts = parts.filter(
    (p) => p.type === "text" && p.text
  );

  // Tool parts: the real API uses type: "tool"
  const toolParts = parts.filter(
    (p) => p.type === "tool" || p.type === "tool-call" || p.type === "tool_call"
  );

  // Step markers (optional – we could render these as group headers)
  // const stepStarts = parts.filter((p) => p.type === "step-start");
  // const stepFinishes = parts.filter((p) => p.type === "step-finish");

  const combinedText = textParts.map((p) => p.text || "").join("\n");

  return (
    <div className={`message-turn message-turn-${role}${isContinuation ? " message-turn-continuation" : ""}`}>
      {/* Avatar — hidden for consecutive messages with the same role */}
      {isContinuation ? (
        <div className="message-avatar-spacer" />
      ) : (
        <div className={`message-avatar ${role}`}>
          {isUser ? <User size={16} /> : isAssistant ? <Bot size={16} /> : <Wrench size={16} />}
        </div>
      )}

      {/* Content */}
      <div className="message-content">
        {/* Role label — hidden for continuations */}
        {!isContinuation && (
          <div className="message-header">
            <span className="message-role">
              {isUser ? "You" : isAssistant ? "Assistant" : role}
            </span>
            {info.model && (
              <span className="message-model">{modelLabel(info.model)}</span>
            )}
            {message.metadata?.cost != null && message.metadata.cost > 0 && (
              <span className="message-cost">
                ${message.metadata.cost.toFixed(4)}
              </span>
            )}
          </div>
        )}
        {/* Cost on continuation messages — show inline if present */}
        {isContinuation && message.metadata?.cost != null && message.metadata.cost > 0 && (
          <div className="message-header message-header-minimal">
            {info.model && (
              <span className="message-model">{modelLabel(info.model)}</span>
            )}
            <span className="message-cost">
              ${message.metadata.cost.toFixed(4)}
            </span>
          </div>
        )}

        {/* Text content with markdown rendering */}
        {combinedText && (
          <div className="message-body">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={{
                code({ className, children, ...props }) {
                  const match = /language-(\w+)/.exec(className || "");
                  const codeStr = String(children).replace(/\n$/, "");
                  if (match) {
                    return (
                      <div className="code-block-wrapper">
                        <div className="code-block-header">
                          <span>{match[1]}</span>
                          <button
                            className="code-copy-btn"
                            onClick={() =>
                              navigator.clipboard.writeText(codeStr)
                            }
                          >
                            Copy
                          </button>
                        </div>
                        <SyntaxHighlighter
                          style={oneDark}
                          language={match[1]}
                          PreTag="div"
                          customStyle={{
                            margin: 0,
                            borderRadius: "0 0 6px 6px",
                            fontSize: "0.8125rem",
                          }}
                        >
                          {codeStr}
                        </SyntaxHighlighter>
                      </div>
                    );
                  }
                  return (
                    <code className="inline-code" {...props}>
                      {children}
                    </code>
                  );
                },
              }}
            >
              {combinedText}
            </ReactMarkdown>
          </div>
        )}

        {/* Tool calls – each tool part is self-contained with input+output in state */}
        {toolParts.map((tc, idx) => (
          <ToolCall
            key={tc.callID || tc.toolCallId || idx}
            part={tc}
          />
        ))}
      </div>
    </div>
  );
}
