import React from "react";
import type { ProjectInfo, SessionStats } from "./api";
import {
  PanelLeft,
  Terminal,
  Command,
  GitBranch,
  FileCode,
  Zap,
  DollarSign,
} from "lucide-react";

interface Props {
  project: ProjectInfo | null;
  stats: SessionStats | null;
  sessionStatus: "idle" | "busy";
  sidebarOpen: boolean;
  terminalOpen: boolean;
  neovimOpen: boolean;
  gitOpen: boolean;
  onToggleSidebar: () => void;
  onToggleTerminal: () => void;
  onToggleNeovim: () => void;
  onToggleGit: () => void;
  onOpenCommandPalette: () => void;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

export function StatusBar({
  project,
  stats,
  sessionStatus,
  sidebarOpen,
  terminalOpen,
  neovimOpen,
  gitOpen,
  onToggleSidebar,
  onToggleTerminal,
  onToggleNeovim,
  onToggleGit,
  onOpenCommandPalette,
}: Props) {
  const totalTokens = stats
    ? stats.input_tokens +
      stats.output_tokens +
      stats.reasoning_tokens +
      stats.cache_read +
      stats.cache_write
    : 0;

  return (
    <div className="chat-status-bar">
      {/* Left section */}
      <div className="status-bar-left">
        <button
          className={`status-bar-btn ${sidebarOpen ? "active" : ""}`}
          onClick={onToggleSidebar}
          title="Toggle Sidebar (Cmd+B)"
        >
          <PanelLeft size={13} />
        </button>

        {project && (
          <span className="status-bar-project">{project.name}</span>
        )}

        {project?.git_branch && (
          <span className="status-bar-branch">
            <GitBranch size={11} />
            {project.git_branch}
          </span>
        )}

        <span
          className={`status-bar-dot ${sessionStatus === "busy" ? "busy" : "idle"}`}
        />
        <span className="status-bar-status">
          {sessionStatus === "busy" ? "busy" : "ready"}
        </span>
      </div>

      {/* Right section */}
      <div className="status-bar-right">
        {stats && totalTokens > 0 && (
          <span className="status-bar-tokens">
            <Zap size={11} />
            {formatTokens(totalTokens)}
          </span>
        )}
        {stats && stats.cost > 0 && (
          <span className="status-bar-cost">
            <DollarSign size={11} />
            {stats.cost.toFixed(4)}
          </span>
        )}

        <button
          className={`status-bar-btn ${neovimOpen ? "active" : ""}`}
          onClick={onToggleNeovim}
          title="Toggle Neovim (Cmd+Shift+E)"
        >
          <FileCode size={13} />
        </button>

        <button
          className={`status-bar-btn ${gitOpen ? "active" : ""}`}
          onClick={onToggleGit}
          title="Toggle Git (Cmd+Shift+G)"
        >
          <GitBranch size={13} />
        </button>

        <button
          className={`status-bar-btn ${terminalOpen ? "active" : ""}`}
          onClick={onToggleTerminal}
          title="Toggle Terminal (Cmd+`)"
        >
          <Terminal size={13} />
        </button>

        <button
          className="status-bar-btn"
          onClick={onOpenCommandPalette}
          title="Command Palette (Cmd+Shift+P)"
        >
          <Command size={13} />
        </button>
      </div>
    </div>
  );
}
