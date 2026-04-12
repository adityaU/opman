import type { MessagePart, Message } from "../types";

export interface ToolCallProps {
  part: MessagePart;
  /** Pre-matched child session for task tools (matched by parent in MessageTurn) */
  childSession?: { id: string; title: string } | null;
  /** SSE-driven subagent messages, keyed by session ID */
  subagentMessages?: Map<string, Message[]>;
  /** Callback to navigate to a child session */
  onOpenSession?: (sessionId: string) => void;
}

export interface TodoItem {
  content: string;
  status: "pending" | "in_progress" | "completed" | "cancelled";
  priority?: "high" | "medium" | "low";
}

export interface ParsedOutput {
  type: "file" | "markdown" | "plain";
  content: string;
  path: string;
}

/** Regex patterns for detecting structured output */
export const FILE_CONTENT_RE =
  /<path>(.*?)<\/path>[\s\S]*?<content>([\s\S]*?)<\/content>/;
export const TASK_RESULT_RE = /<task_result>([\s\S]*?)<\/task_result>/;

/** Extension → syntax highlighter language mapping */
export const EXT_TO_LANG: Record<string, string> = {
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
