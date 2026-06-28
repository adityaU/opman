import React, { useState, useEffect, useMemo } from "react";
import { fetchCommands } from "./api";
import type { SlashCommand } from "./types";

interface Props {
  filter: string;
  onSelect: (command: string) => void;
  onClose: () => void;
  sessionId: string | null;
  /** Active engine backend ("opencode" | "claude-code"). */
  backend?: string;
}

/**
 * Built-in commands that are sent to the agent server and are specific to
 * opencode — they have no equivalent in the Claude engine, so they are hidden
 * when the backend is claude-code.
 */
const OPENCODE_ONLY_COMMANDS = new Set(["undo", "redo", "fork", "share"]);

/**
 * Built-in commands that are always available.
 * These are handled locally by the web UI or are built-in opencode
 * server commands that don't appear in the /command API listing.
 */
const BUILTIN_COMMANDS: SlashCommand[] = [
  // Session lifecycle
  { name: "new", description: "Start a new session" },
  { name: "cancel", description: "Cancel / abort the running session" },
  { name: "copy", description: "Copy session transcript to clipboard" },
  { name: "compact", description: "Compact conversation history" },
  { name: "undo", description: "Undo last action" },
  { name: "redo", description: "Redo last action" },
  { name: "fork", description: "Fork current session" },
  { name: "share", description: "Share session" },
  { name: "clear", description: "Clear conversation" },
  // Model / agent
  { name: "model", description: "Change the AI model", args: "<model>" },
  { name: "models", description: "List available models" },
  { name: "agent", description: "Switch agent type", args: "<agent>" },
  { name: "theme", description: "Change color theme", args: "<theme>" },
  // Panel toggles
  { name: "terminal", description: "Toggle terminal panel" },
  { name: "neovim", description: "Toggle Neovim panel" },
  { name: "git", description: "Toggle Git panel" },
  { name: "split-view", description: "Toggle split view" },
  { name: "debug", description: "Toggle debug panel" },
  // Modal commands
  { name: "keys", description: "Show keyboard shortcuts" },
  { name: "todos", description: "Show session todos" },
  { name: "sessions", description: "Search sessions across projects" },
  { name: "context", description: "Send context to session" },
  { name: "settings", description: "Open settings" },
  { name: "watcher", description: "Open file watcher config" },
  { name: "search", description: "Search current session" },
  { name: "cross-search", description: "Search across all sessions" },
  { name: "context-window", description: "View context window usage" },
  { name: "diff-review", description: "Review pending diffs" },
  { name: "auto-open", description: "Configure tool auto-open" },
  // Assistant / delegation
  { name: "assistant-center", description: "Open the assistant cockpit" },
  { name: "inbox", description: "Open the assistant inbox" },
  { name: "missions", description: "Open mission tracking" },
  { name: "memory", description: "Open personal memory" },
  { name: "autonomy", description: "Adjust assistant autonomy" },
  { name: "routines", description: "Manage assistant routines" },
  { name: "delegation", description: "Open delegation board" },
  { name: "workspaces", description: "Open workspaces and recipes" },
  // System
  { name: "system", description: "Open system monitor (htop)" },
  { name: "health", description: "View process health" },
  // Analytics
  { name: "session-graph", description: "View session dependency graph" },
  { name: "session-dashboard", description: "View session analytics" },
  { name: "activity-feed", description: "View activity feed" },
  { name: "notification-prefs", description: "Notification preferences" },
];

export function SlashCommandPopover({
  filter,
  onSelect,
  onClose,
  sessionId,
  backend,
}: Props) {
  const [apiCommands, setApiCommands] = useState<SlashCommand[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    fetchCommands()
      .then((cmds) => {
        if (cmds.length > 0) setApiCommands(cmds);
      })
      .catch(() => {});
  }, [sessionId]);

  // Merge built-in + API commands, deduplicating by name (built-in wins).
  // Under the Claude engine, drop opencode-only commands (the API list provides
  // Claude's own slash commands instead).
  const commands = useMemo(() => {
    const isClaude = backend === "claude-code";
    const builtins = isClaude
      ? BUILTIN_COMMANDS.filter((c) => !OPENCODE_ONLY_COMMANDS.has(c.name))
      : BUILTIN_COMMANDS;
    const builtinNames = new Set(builtins.map((c) => c.name));
    const apiOnly = apiCommands.filter((c) => !builtinNames.has(c.name));
    return [...builtins, ...apiOnly];
  }, [apiCommands, backend]);

  const filtered = useMemo(() => {
    if (!filter) return commands;
    const lf = filter.toLowerCase();
    return commands.filter(
      (c) =>
        c.name.toLowerCase().includes(lf) ||
        c.description?.toLowerCase().includes(lf)
    );
  }, [commands, filter]);

  // Reset selection when filter changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [filter]);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        if (filtered[selectedIndex]) {
          onSelect(filtered[selectedIndex].name);
        }
      } else if (e.key === "Escape") {
        onClose();
      }
    }
    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [filtered, selectedIndex, onSelect, onClose]);

  if (filtered.length === 0) return null;

  return (
    <div className="slash-popover">
      {filtered.map((cmd, idx) => (
        <button
          key={cmd.name}
          className={`slash-popover-item ${idx === selectedIndex ? "selected" : ""}`}
          onClick={() => onSelect(cmd.name)}
          onMouseEnter={() => setSelectedIndex(idx)}
        >
          <span className="slash-popover-name">/{cmd.name}</span>
          {cmd.description && (
            <span className="slash-popover-desc">{cmd.description}</span>
          )}
          {cmd.args && (
            <span className="slash-popover-args">{cmd.args}</span>
          )}
        </button>
      ))}
    </div>
  );
}
