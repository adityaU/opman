import React from "react";
import type { ProjectInfo, SessionStats, ClientPresence } from "./api";
import type { WatcherStatus, SSEConnectionStatus } from "./hooks/useSSE";
import { WatcherStatusIndicator } from "./WatcherStatusBar";
import {
  PanelLeft,
  Terminal,
  Command,
  GitBranch,
  FileCode,
  Zap,
  DollarSign,
  Users,
  Layers,
  Wifi,
  WifiOff,
} from "lucide-react";

interface Props {
  project: ProjectInfo | null;
  stats: SessionStats | null;
  connectionStatus?: SSEConnectionStatus;
  sidebarOpen: boolean;
  terminalOpen: boolean;
  neovimOpen: boolean;
  gitOpen: boolean;
  watcherStatus: WatcherStatus | null;
  /** Connected clients for presence tracking. */
  presenceClients?: ClientPresence[];
  /** Name of the currently active workspace (if any). */
  activeWorkspaceName?: string | null;
  contextLimit: number | null;
  onToggleSidebar: () => void;
  sessionTitle?: string | null;
  showSidebarToggle?: boolean;
  onToggleTerminal: () => void;
  onToggleNeovim: () => void;
  onToggleGit: () => void;
  onOpenCommandPalette: () => void;
  onOpenWatcher: () => void;
  onOpenContextWindow: () => void;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

export const StatusBar = React.memo(function StatusBar({
  project,
  stats,
  connectionStatus,
  sidebarOpen,
  terminalOpen,
  neovimOpen,
  gitOpen,
  watcherStatus,
  presenceClients,
  activeWorkspaceName,
  contextLimit,
  onToggleSidebar,
  sessionTitle,
  showSidebarToggle = false,
  onToggleTerminal,
  onToggleNeovim,
  onToggleGit,
  onOpenCommandPalette,
  onOpenWatcher,
  onOpenContextWindow,
}: Props) {
  const totalTokens = stats
    ? stats.input_tokens +
      stats.output_tokens +
      stats.reasoning_tokens +
      stats.cache_read +
      stats.cache_write
    : 0;

  // Context window usage percentage and color class
  const contextPct = contextLimit && totalTokens > 0
    ? Math.round((totalTokens / contextLimit) * 100)
    : null;
  const contextColorClass = contextPct !== null
    ? contextPct > 90 ? "context-critical" : contextPct > 70 ? "context-warning" : "context-ok"
    : "";

  return (
    <div className="chat-header">
      {/* Left section */}
      <div className="status-bar-left">
        {showSidebarToggle && (
          <button
            className="status-bar-btn"
            onClick={onToggleSidebar}
            title="Show sidebar (Cmd+B)"
            aria-label="Show sidebar"
          >
            <PanelLeft size={13} />
          </button>
        )}
        {sessionTitle && <span className="chat-header-title" title={sessionTitle}>{sessionTitle}</span>}

        {project && (
          <span className="status-bar-project">{project.name}</span>
        )}

        {project?.git_branch && (
          <span className="status-bar-branch">
            <GitBranch size={11} />
            {project.git_branch}
          </span>
        )}

        {connectionStatus && connectionStatus !== "connected" && (
          <span
            className={`status-bar-connection status-bar-connection-${connectionStatus}`}
            role="status"
            aria-label={connectionStatus === "reconnecting" ? "Reconnecting to server" : "Disconnected from server"}
            title={connectionStatus === "reconnecting" ? "Reconnecting..." : "Disconnected"}
          >
            <WifiOff size={11} />
            <span>{connectionStatus === "reconnecting" ? "reconnecting" : "disconnected"}</span>
          </span>
        )}

        {watcherStatus && (
          <WatcherStatusIndicator
            watcherStatus={watcherStatus}
            onClick={onOpenWatcher}
          />
        )}

        {presenceClients && presenceClients.length > 1 && (
          <span
            className="status-bar-presence"
            title={`${presenceClients.length} connected client${presenceClients.length !== 1 ? "s" : ""}: ${presenceClients.map((c) => c.interface_type).join(", ")}`}
          >
            <Users size={11} />
            <span>{presenceClients.length}</span>
          </span>
        )}

        {activeWorkspaceName && (
          <span className="status-bar-workspace" title={`Workspace: ${activeWorkspaceName}`}>
            <Layers size={11} />
            <span>{activeWorkspaceName}</span>
          </span>
        )}

      </div>

      {/* Right section */}
      <div className="status-bar-right">
        {stats && totalTokens > 0 && (
          <button
            className={`status-bar-tokens status-bar-tokens-btn ${contextColorClass}`}
            title={contextLimit
              ? `${totalTokens.toLocaleString()} / ${contextLimit.toLocaleString()} tokens (${contextPct}%) — Click for details`
              : `${totalTokens.toLocaleString()} tokens — Click for details`}
            onClick={onOpenContextWindow}
          >
            <Zap size={11} />
            {formatTokens(totalTokens)}
            {contextLimit && (
              <span className="status-bar-token-limit"> / {formatTokens(contextLimit)}</span>
            )}
            {contextPct !== null && (
              <span className="status-bar-token-pct"> {contextPct}%</span>
            )}
          </button>
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
          title="Toggle Editor (Cmd+Shift+E)"
          aria-label="Toggle editor"
        >
          <FileCode size={13} />
        </button>

        <button
          className={`status-bar-btn ${gitOpen ? "active" : ""}`}
          onClick={onToggleGit}
          title="Toggle Git (Cmd+Shift+G)"
          aria-label="Toggle git"
        >
          <GitBranch size={13} />
        </button>

        <button
          className={`status-bar-btn ${terminalOpen ? "active" : ""}`}
          onClick={onToggleTerminal}
          title="Toggle Terminal (Cmd+`)"
          aria-label="Toggle terminal"
        >
          <Terminal size={13} />
        </button>

        <button
          className="status-bar-btn"
          onClick={onOpenCommandPalette}
          title="Command Palette (Cmd+Shift+P)"
          aria-label="Command palette"
        >
          <Command size={13} />
        </button>
      </div>
    </div>
  );
});
