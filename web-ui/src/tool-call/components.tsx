import React, { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import {
  Check,
  Minus,
  Circle,
  CircleDot,
  Plus,
  Loader2,
} from "lucide-react";
import { TodoItem } from "./types";
import { parseOutput, guessLanguage } from "./helpers";

// ── ToolInput: Syntax-highlighted JSON or plain text ────

export function ToolInput({ data }: { data: Record<string, unknown> | string }) {
  const formatted = useMemo(() => {
    if (typeof data === "string") return data;
    return JSON.stringify(data, null, 2);
  }, [data]);

  const isJson = typeof data !== "string";

  if (isJson) {
    return (
      <SyntaxHighlighter
        useInlineStyles={false}
        language="json"
        PreTag="div"
        customStyle={{
          margin: 0,
          borderRadius: "4px",
          fontSize: "0.75rem",
          maxHeight: "300px",
          overflow: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {formatted}
      </SyntaxHighlighter>
    );
  }

  return <pre className="tool-call-pre">{formatted}</pre>;
}

// ── ToolOutput: Smart rendering based on content format ──

export function ToolOutput({
  output,
  toolName,
  isLive,
}: {
  output: string;
  toolName: string;
  isLive?: boolean;
}) {
  const parsed = useMemo(() => parseOutput(output), [output]);
  const liveRef = React.useRef<HTMLPreElement>(null);

  // Auto-scroll live output to bottom
  React.useEffect(() => {
    if (isLive && liveRef.current) {
      liveRef.current.scrollTop = liveRef.current.scrollHeight;
    }
  }, [isLive, output]);

  if (parsed.type === "file") {
    const lang = guessLanguage(parsed.path);
    return (
      <div className="tool-output-file">
        <div className="tool-output-file-header">
          <span className="tool-output-file-path">{parsed.path}</span>
        </div>
        <SyntaxHighlighter
          useInlineStyles={false}
          language={lang}
          PreTag="div"
          showLineNumbers
          customStyle={{
            margin: 0,
            borderRadius: "0 0 4px 4px",
            fontSize: "0.75rem",
            maxHeight: "400px",
            overflow: "auto",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }}
        >
          {parsed.content}
        </SyntaxHighlighter>
      </div>
    );
  }

  if (parsed.type === "markdown") {
    return (
      <div className="tool-output-markdown">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>
          {parsed.content}
        </ReactMarkdown>
      </div>
    );
  }

  // Plain text output
  return (
    <pre ref={liveRef} className={`tool-call-pre${isLive ? " tool-call-live-output" : ""}`}>
      {output}
    </pre>
  );
}

// ── TodoList: Render todowrite input as checklist ────────

export function TodoList({ input }: { input: Record<string, unknown> | string }) {
  const todos = useMemo(() => {
    try {
      const data = typeof input === "string" ? JSON.parse(input) : input;
      const items = data?.todos || data;
      if (!Array.isArray(items)) return [];
      return items as TodoItem[];
    } catch {
      return [];
    }
  }, [input]);

  if (todos.length === 0) {
    return <pre className="tool-call-pre tool-call-empty">No todos</pre>;
  }

  const counts = {
    completed: todos.filter((t) => t.status === "completed").length,
    total: todos.length,
  };

  return (
    <div className="todo-list">
      {todos.map((todo, idx) => (
        <div key={idx} className="todo-item">
          <span className={`todo-checkbox ${todo.status}`}>
            {todo.status === "completed" ? (
              <Check size={10} />
            ) : todo.status === "in_progress" ? (
              <CircleDot size={10} />
            ) : todo.status === "cancelled" ? (
              <Minus size={10} />
            ) : (
              <Circle size={8} />
            )}
          </span>
          <span className={`todo-content ${todo.status}`}>{todo.content}</span>
          {todo.priority && (
            <span className={`todo-priority ${todo.priority}`}>
              {todo.priority}
            </span>
          )}
        </div>
      ))}
      <div
        style={{
          fontSize: "var(--font-size-2xs)",
          color: "var(--color-text-muted)",
          padding: "var(--space-1) var(--space-2)",
          borderTop: "1px solid var(--color-border-subtle)",
          marginTop: "var(--space-1)",
        }}
      >
        {counts.completed}/{counts.total} completed
      </div>
    </div>
  );
}

// ── EditDiffView: Render edit tool input as a diff ──────

export function EditDiffView({ input }: { input: Record<string, unknown> | string }) {
  const diff = useMemo(() => {
    try {
      const data = typeof input === "string" ? JSON.parse(input) : input;
      const filePath = (data?.filePath || data?.file_path || data?.path || "") as string;
      const oldStr = (data?.oldString || data?.old_string || "") as string;
      const newStr = (data?.newString || data?.new_string || "") as string;

      if (!oldStr && !newStr) return null;
      return { filePath, oldStr, newStr };
    } catch {
      return null;
    }
  }, [input]);

  if (!diff) {
    return <ToolInput data={input} />;
  }

  const oldLines = diff.oldStr.split("\n");
  const newLines = diff.newStr.split("\n");

  return (
    <div className="diff-view">
      {diff.filePath && (
        <div className="diff-header">
          <span className="diff-file-path">{diff.filePath}</span>
        </div>
      )}
      {oldLines.map((line, i) => (
        <div key={`old-${i}`} className="diff-line removed">
          <span className="diff-line-num">{i + 1}</span>
          <span className="diff-line-content">- {line}</span>
        </div>
      ))}
      {newLines.map((line, i) => (
        <div key={`new-${i}`} className="diff-line added">
          <span className="diff-line-num">{i + 1}</span>
          <span className="diff-line-content">+ {line}</span>
        </div>
      ))}
      <div className="diff-stats">
        <span className="diff-stats-removed">
          <Minus size={10} /> {oldLines.length} removed
        </span>
        <span className="diff-stats-added">
          <Plus size={10} /> {newLines.length} added
        </span>
      </div>
    </div>
  );
}
