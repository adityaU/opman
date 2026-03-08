import React, { useState, useEffect, useMemo } from "react";
import { fetchCommands } from "./api";
import type { SlashCommand } from "./types";

interface Props {
  filter: string;
  onSelect: (command: string) => void;
  onClose: () => void;
  sessionId: string | null;
}

/**
 * Built-in commands that are always available.
 * These are handled locally by the web UI or are built-in opencode
 * server commands that don't appear in the /command API listing.
 */
const BUILTIN_COMMANDS: SlashCommand[] = [
  // Web-local commands
  { name: "new", description: "Start a new session" },
  { name: "terminal", description: "Toggle terminal panel" },
  // Built-in opencode server commands (not in /command API but valid)
  { name: "model", description: "Change the AI model", args: "<model>" },
  { name: "models", description: "List available models" },
  { name: "theme", description: "Change color theme", args: "<theme>" },
  { name: "compact", description: "Compact conversation history" },
  { name: "undo", description: "Undo last action" },
  { name: "redo", description: "Redo last action" },
  { name: "fork", description: "Fork current session" },
  { name: "share", description: "Share session" },
  { name: "agent", description: "Switch agent type", args: "<agent>" },
  { name: "clear", description: "Clear conversation" },
  // Modal commands
  { name: "keys", description: "Show keyboard shortcuts" },
  { name: "todos", description: "Show session todos" },
  { name: "sessions", description: "Search sessions across projects" },
  { name: "context", description: "Send context to session" },
  { name: "settings", description: "Open settings" },
  { name: "assistant-center", description: "Open the assistant cockpit" },
  { name: "inbox", description: "Open the assistant inbox" },
  { name: "missions", description: "Open mission tracking" },
  { name: "memory", description: "Open personal memory" },
  { name: "autonomy", description: "Adjust assistant autonomy" },
  { name: "routines", description: "Manage assistant routines" },
  { name: "delegation", description: "Open delegation board" },
  { name: "workspaces", description: "Open workspaces and recipes" },
];

export function SlashCommandPopover({
  filter,
  onSelect,
  onClose,
  sessionId,
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

  // Merge built-in + API commands, deduplicating by name (built-in wins)
  const commands = useMemo(() => {
    const builtinNames = new Set(BUILTIN_COMMANDS.map((c) => c.name));
    const apiOnly = apiCommands.filter((c) => !builtinNames.has(c.name));
    return [...BUILTIN_COMMANDS, ...apiOnly];
  }, [apiCommands]);

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
