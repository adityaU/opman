import React, { useState, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { MessagePart } from "./types";
import {
  ChevronDown,
  ChevronRight,
  Wrench,
  CheckCircle2,
  XCircle,
  Loader2,
  Clock,
  Check,
  Minus,
  Circle,
  CircleDot,
  Plus,
} from "lucide-react";

interface Props {
  part: MessagePart;
}

/**
 * Renders a tool call part with syntax-highlighted input/output.
 *
 * The API returns parts with `type: "tool"` containing:
 *   - tool: string (tool name)
 *   - callID: string
 *   - state: { input, output, status, title, time, metadata, attachments }
 *
 * Output formats detected:
 *   1. XML file content: <path>...</path><type>file</type><content>...</content>
 *   2. Task result markdown: <task_result>markdown</task_result>
 *   3. Plain text
 */
export function ToolCall({ part }: Props) {
  const toolName = part.tool || part.toolName || "unknown";
  const shortName = formatToolName(toolName);

  // Detect special tool types
  const isTodoWrite = toolName.includes("todowrite") || toolName.includes("todo_write");

  const [expanded, setExpanded] = useState(isTodoWrite);

  const state = part.state;
  const status = state?.status || "pending";
  const isError = status === "error";
  const isCompleted = status === "completed";
  const isRunning = status === "running" || status === "pending";
  const isEditTool = toolName.includes("edit") && !toolName.includes("neovim");

  const durationMs =
    state?.time?.start && state?.time?.end
      ? state.time.end - state.time.start
      : null;

  const inputData = state?.input;
  const hasInput =
    inputData != null &&
    (typeof inputData === "string"
      ? inputData.length > 0
      : Object.keys(inputData).length > 0);

  const outputData = state?.output;
  const hasOutput = outputData != null && outputData.length > 0;

  return (
    <div className={`tool-call ${isError ? "tool-call-error" : ""}`}>
      <button
        className="tool-call-header"
        onClick={() => setExpanded(!expanded)}
      >
        <span className="tool-call-icon">
          {expanded ? (
            <ChevronDown size={14} />
          ) : (
            <ChevronRight size={14} />
          )}
        </span>
        <Wrench size={12} />
        <span className="tool-call-name">{shortName}</span>
        {state?.title && (
          <span className="tool-call-title">{state.title}</span>
        )}
        <span className="tool-call-status">
          {durationMs != null && (
            <span className="tool-call-duration">
              <Clock size={10} />
              {formatDuration(durationMs)}
            </span>
          )}
          {isCompleted ? (
            <CheckCircle2 size={12} className="tool-success-icon" />
          ) : isError ? (
            <XCircle size={12} className="tool-error-icon" />
          ) : isRunning ? (
            <span className="tool-call-pending">
              <Loader2 size={12} className="tool-spin-icon" /> running...
            </span>
          ) : (
            <span className="tool-call-pending">{status}</span>
          )}
        </span>
      </button>

      {expanded && (
        <div className="tool-call-body">
          {/* TodoWrite: render as checklist instead of raw JSON */}
          {isTodoWrite && hasInput ? (
            <div className="tool-call-section">
              <div className="tool-call-section-label">Todos</div>
              <TodoList input={inputData!} />
            </div>
          ) : (
            <>
              {/* Input / Arguments — syntax highlighted JSON or diff */}
              {hasInput && (
                <div className="tool-call-section">
                  <div className="tool-call-section-label">Input</div>
                  {isEditTool ? (
                    <EditDiffView input={inputData!} />
                  ) : (
                    <ToolInput data={inputData!} />
                  )}
                </div>
              )}

              {/* Output / Result — smart rendering */}
              {hasOutput && (
                <div className="tool-call-section">
                  <div className="tool-call-section-label">Output</div>
                  {state?.metadata?.truncated && (
                    <span className="tool-call-truncated">[truncated] </span>
                  )}
                  <ToolOutput output={outputData!} toolName={toolName} />
                </div>
              )}
            </>
          )}

          {!isTodoWrite && !hasInput && !hasOutput && (
            <div className="tool-call-section">
              <pre className="tool-call-pre tool-call-empty">
                No data available
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── ToolInput: Syntax-highlighted JSON or plain text ────

function ToolInput({ data }: { data: Record<string, unknown> | string }) {
  const formatted = useMemo(() => {
    if (typeof data === "string") return data;
    return JSON.stringify(data, null, 2);
  }, [data]);

  const isJson = typeof data !== "string";

  if (isJson) {
    return (
      <SyntaxHighlighter
        style={oneDark}
        language="json"
        PreTag="div"
        customStyle={{
          margin: 0,
          borderRadius: "4px",
          fontSize: "0.75rem",
          maxHeight: "300px",
          overflow: "auto",
        }}
      >
        {formatted}
      </SyntaxHighlighter>
    );
  }

  return <pre className="tool-call-pre">{formatted}</pre>;
}

// ── ToolOutput: Smart rendering based on content format ──

/** Regex patterns for detecting structured output */
const FILE_CONTENT_RE =
  /<path>(.*?)<\/path>[\s\S]*?<content>([\s\S]*?)<\/content>/;
const TASK_RESULT_RE = /<task_result>([\s\S]*?)<\/task_result>/;

function ToolOutput({
  output,
  toolName,
}: {
  output: string;
  toolName: string;
}) {
  const parsed = useMemo(() => parseOutput(output), [output]);

  if (parsed.type === "file") {
    const lang = guessLanguage(parsed.path);
    return (
      <div className="tool-output-file">
        <div className="tool-output-file-header">
          <span className="tool-output-file-path">{parsed.path}</span>
        </div>
        <SyntaxHighlighter
          style={oneDark}
          language={lang}
          PreTag="div"
          showLineNumbers
          customStyle={{
            margin: 0,
            borderRadius: "0 0 4px 4px",
            fontSize: "0.75rem",
            maxHeight: "400px",
            overflow: "auto",
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
  return <pre className="tool-call-pre">{output}</pre>;
}

// ── TodoList: Render todowrite input as checklist ────────

interface TodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed" | "cancelled";
  priority?: "high" | "medium" | "low";
}

function TodoList({ input }: { input: Record<string, unknown> | string }) {
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

function EditDiffView({ input }: { input: Record<string, unknown> | string }) {
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
    // Fallback to regular JSON display
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
      {/* Removed lines */}
      {oldLines.map((line, i) => (
        <div key={`old-${i}`} className="diff-line removed">
          <span className="diff-line-num">{i + 1}</span>
          <span className="diff-line-content">- {line}</span>
        </div>
      ))}
      {/* Added lines */}
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

// ── Output parsing ───────────────────────────────────

interface ParsedOutput {
  type: "file" | "markdown" | "plain";
  content: string;
  path: string;
}

function parseOutput(output: string): ParsedOutput {
  // Check for file content XML pattern
  const fileMatch = FILE_CONTENT_RE.exec(output);
  if (fileMatch) {
    return {
      type: "file",
      path: fileMatch[1],
      content: fileMatch[2],
    };
  }

  // Check for task_result markdown pattern
  const taskMatch = TASK_RESULT_RE.exec(output);
  if (taskMatch) {
    return {
      type: "markdown",
      path: "",
      content: taskMatch[1].trim(),
    };
  }

  return { type: "plain", path: "", content: output };
}

// ── Language detection from file extension ────────────

const EXT_TO_LANG: Record<string, string> = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  jsx: "jsx",
  py: "python",
  rs: "rust",
  go: "go",
  rb: "ruby",
  java: "java",
  kt: "kotlin",
  swift: "swift",
  c: "c",
  cpp: "cpp",
  h: "c",
  hpp: "cpp",
  cs: "csharp",
  css: "css",
  scss: "scss",
  html: "html",
  xml: "xml",
  json: "json",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  md: "markdown",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  sql: "sql",
  lua: "lua",
  vim: "vim",
  dockerfile: "dockerfile",
  makefile: "makefile",
};

function guessLanguage(path: string): string {
  const filename = path.split("/").pop() || "";
  const lower = filename.toLowerCase();

  // Special filenames
  if (lower === "dockerfile") return "dockerfile";
  if (lower === "makefile" || lower === "gnumakefile") return "makefile";
  if (lower.endsWith(".lock")) return "json";

  const ext = lower.split(".").pop() || "";
  return EXT_TO_LANG[ext] || "text";
}

// ── Helper functions ─────────────────────────────────

/** Shorten tool names by removing common prefixes (provider_provider_action -> action) */
function formatToolName(name: string): string {
  const parts = name.split("_");
  if (parts.length >= 3 && parts[0] === parts[1]) {
    return parts.slice(2).join("_");
  }
  if (parts.length === 2 && parts[0] === parts[1]) {
    return parts[0];
  }
  return name;
}

/** Format milliseconds as a human-readable duration */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return `${m}m ${rem.toFixed(0)}s`;
}
