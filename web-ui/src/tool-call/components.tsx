import React, { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
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
import { markdownComponents, REMARK_PLUGINS } from "../message-turn/CodeBlock";

// ── ToolInput: Syntax-highlighted JSON or plain text ────

/** Stable style objects to avoid allocations per render */
const TOOL_INPUT_STYLE = {
  margin: 0,
  borderRadius: "var(--radius)",
  fontSize: "0.75rem",
  maxHeight: "300px",
  overflow: "auto",
  whiteSpace: "pre-wrap" as const,
  wordBreak: "break-word" as const,
  fontFamily: "var(--font-mono)",
};
const TOOL_CODE_TAG_PROPS = { style: { fontFamily: "var(--font-mono)" } };
const TOOL_OUTPUT_FILE_STYLE = {
  margin: 0,
  borderRadius: "0 0 var(--radius) var(--radius)",
  fontSize: "0.75rem",
  maxHeight: "400px",
  overflow: "auto",
  whiteSpace: "pre-wrap" as const,
  wordBreak: "break-word" as const,
  fontFamily: "var(--font-mono)",
};

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
        codeTagProps={TOOL_CODE_TAG_PROPS}
        customStyle={TOOL_INPUT_STYLE}
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
          codeTagProps={TOOL_CODE_TAG_PROPS}
          customStyle={TOOL_OUTPUT_FILE_STYLE}
        >
          {parsed.content}
        </SyntaxHighlighter>
      </div>
    );
  }

  if (parsed.type === "markdown") {
    return (
      <div className="tool-output-markdown">
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
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

// ── EditDiffView: render edit-tool input as a real line diff ──────
//
// Handles all engines/shapes: Edit (old_string/new_string), Write (content → a new
// file, all additions), and MultiEdit (edits: [{old_string,new_string}]). Field names
// are matched in both snake_case (claude/claudep) and camelCase (opencode). The diff is
// computed with an LCS line-diff so only changed lines are colored — unchanged lines
// render as neutral context. Colors come from the theme (--color-success/-error) via
// the .diff-line.added/.removed classes.

type DiffRow = { type: "ctx" | "add" | "del"; text: string };

/** LCS-based line diff between two texts. */
function lineDiff(oldText: string, newText: string): DiffRow[] {
  const a = oldText.length ? oldText.split("\n") : [];
  const b = newText.length ? newText.split("\n") : [];
  const m = a.length;
  const n = b.length;
  // dp[i][j] = LCS length of a[i:] and b[j:]
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  while (i < m && j < n) {
    if (a[i] === b[j]) { rows.push({ type: "ctx", text: a[i] }); i++; j++; }
    else if (dp[i + 1][j] >= dp[i][j + 1]) { rows.push({ type: "del", text: a[i] }); i++; }
    else { rows.push({ type: "add", text: b[j] }); j++; }
  }
  while (i < m) rows.push({ type: "del", text: a[i++] });
  while (j < n) rows.push({ type: "add", text: b[j++] });
  return rows;
}

const str = (v: unknown): string => (typeof v === "string" ? v : "");

export function EditDiffView({ input }: { input: Record<string, unknown> | string }) {
  const parsed = useMemo(() => {
    try {
      const data = (typeof input === "string" ? JSON.parse(input) : input) as Record<string, unknown>;
      const filePath = str(data?.filePath || data?.file_path || data?.path);

      // Each edit hunk is an (old → new) pair. Build them per tool shape.
      const hunks: { oldStr: string; newStr: string }[] = [];
      const edits = data?.edits;
      if (Array.isArray(edits)) {
        // MultiEdit
        for (const e of edits as Record<string, unknown>[]) {
          hunks.push({ oldStr: str(e?.oldString || e?.old_string), newStr: str(e?.newString || e?.new_string) });
        }
      } else if (data?.content !== undefined) {
        // Write — a new/replacement file: everything is an addition.
        hunks.push({ oldStr: "", newStr: str(data.content) });
      } else {
        // Edit
        hunks.push({ oldStr: str(data?.oldString || data?.old_string), newStr: str(data?.newString || data?.new_string) });
      }

      const rows = hunks.flatMap((h) => lineDiff(h.oldStr, h.newStr));
      if (rows.length === 0) return null;
      const added = rows.filter((r) => r.type === "add").length;
      const removed = rows.filter((r) => r.type === "del").length;
      return { filePath, rows, added, removed };
    } catch {
      return null;
    }
  }, [input]);

  if (!parsed) {
    return <ToolInput data={input} />;
  }

  const cls = (t: DiffRow["type"]) =>
    t === "add" ? "diff-line added" : t === "del" ? "diff-line removed" : "diff-line";
  const sign = (t: DiffRow["type"]) => (t === "add" ? "+" : t === "del" ? "-" : " ");

  return (
    <div className="diff-view">
      {parsed.filePath && (
        <div className="diff-header">
          <span className="diff-file-path">{parsed.filePath}</span>
        </div>
      )}
      {parsed.rows.map((row, i) => (
        <div key={i} className={cls(row.type)}>
          <span className="diff-line-content">{sign(row.type)} {row.text}</span>
        </div>
      ))}
      <div className="diff-stats">
        <span className="diff-stats-removed">
          <Minus size={10} /> {parsed.removed} removed
        </span>
        <span className="diff-stats-added">
          <Plus size={10} /> {parsed.added} added
        </span>
      </div>
    </div>
  );
}
