/**
 * useAutoOpen — localStorage-persisted tool-call accordion auto-open config.
 *
 * Each tool category has a toggle controlling whether its accordion auto-expands
 * when it appears. All toggles default to OFF. Uses `useSyncExternalStore` so
 * any component calling `useAutoOpen()` will re-render when the config changes.
 */

import { useCallback, useSyncExternalStore } from "react";

const STORAGE_KEY = "opman_auto_open_config";

// ── Tool category ────────────────────────────────────────────────

export type ToolCategory =
  | "bash_output"
  | "subagent_task"
  | "edit_tools"
  | "read_tools"
  | "write_tools"
  | "todo_write"
  | "other_tools";

export interface ToolCategoryInfo {
  key: ToolCategory;
  label: string;
  description: string;
  iconPath: string;
}

export const TOOL_CATEGORIES: ToolCategoryInfo[] = [
  {
    key: "bash_output",
    label: "Bash Output",
    description: "Shell, terminal, and bash command output panes",
    iconPath: "M4 17l6-6-6-6 M12 19h8",
  },
  {
    key: "subagent_task",
    label: "Subagent Tasks",
    description: "Task / subagent session cards with nested messages",
    iconPath: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8z M22 21v-2a4 4 0 0 0-3-3.87 M16 3.13a4 4 0 0 1 0 7.75",
  },
  {
    key: "edit_tools",
    label: "Edit Tools",
    description: "File edit tool accordions (edit, replace)",
    iconPath: "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7 M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z",
  },
  {
    key: "read_tools",
    label: "Read Tools",
    description: "File read tools (read, glob, grep, search)",
    iconPath: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
  },
  {
    key: "write_tools",
    label: "Write Tools",
    description: "File write / create tool accordions",
    iconPath: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M16 13H8 M16 17H8 M10 9H8",
  },
  {
    key: "todo_write",
    label: "Todo List",
    description: "Todo list accordion showing task progress",
    iconPath: "M9 11l3 3L22 4 M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11",
  },
  {
    key: "other_tools",
    label: "Other Tools",
    description: "All remaining tools not covered above",
    iconPath: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z",
  },
];

// ── Config type ──────────────────────────────────────────────────

export type AutoOpenConfig = Record<ToolCategory, boolean>;

const DEFAULT_CONFIG: AutoOpenConfig = {
  bash_output: false,
  subagent_task: false,
  edit_tools: false,
  read_tools: false,
  write_tools: false,
  todo_write: false,
  other_tools: false,
};

// ── Classify tool names ──────────────────────────────────────────

/** Classify a tool name into its auto-open category.
 *  Returns null for A2UI tools (inline, no accordion). */
export function classifyTool(toolName: string): ToolCategory | null {
  if (toolName.includes("ui_render") || toolName.includes("ui_ui_render") || toolName === "a2ui") {
    return null;
  }
  if (toolName.includes("todowrite") || toolName.includes("todo_write")) return "todo_write";
  if (toolName === "task") return "subagent_task";
  if (toolName.includes("bash") || toolName.includes("shell") || toolName.includes("terminal")) {
    return "bash_output";
  }
  if (toolName.includes("edit") && !toolName.includes("neovim")) return "edit_tools";
  if (toolName.includes("read") || toolName.includes("glob") || toolName.includes("grep") || toolName.includes("search")) {
    return "read_tools";
  }
  if (toolName.includes("write") || toolName.includes("create")) return "write_tools";
  return "other_tools";
}

// ── Module-level store (shared singleton) ────────────────────────

let _config: AutoOpenConfig = loadConfig();
const _listeners = new Set<() => void>();

function loadConfig(): AutoOpenConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_CONFIG, ...JSON.parse(raw) };
  } catch { /* ignore */ }
  return { ...DEFAULT_CONFIG };
}

function persistConfig(cfg: AutoOpenConfig) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(cfg));
  } catch { /* ignore */ }
}

function notify() {
  for (const fn of _listeners) fn();
}

function getSnapshot(): AutoOpenConfig {
  return _config;
}

function subscribe(cb: () => void): () => void {
  _listeners.add(cb);
  return () => _listeners.delete(cb);
}

// ── Public API ───────────────────────────────────────────────────

export interface AutoOpenAPI {
  config: AutoOpenConfig;
  toggle: (cat: ToolCategory) => void;
  /** Check if a specific tool name should auto-open. */
  shouldAutoOpen: (toolName: string) => boolean;
}

export function useAutoOpen(): AutoOpenAPI {
  const config = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  const toggle = useCallback((cat: ToolCategory) => {
    _config = { ..._config, [cat]: !_config[cat] };
    persistConfig(_config);
    notify();
  }, []);

  const shouldAutoOpen = useCallback(
    (toolName: string) => {
      const cat = classifyTool(toolName);
      if (!cat) return false;
      return config[cat];
    },
    [config],
  );

  return { config, toggle, shouldAutoOpen };
}
