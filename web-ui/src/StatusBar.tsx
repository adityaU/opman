import React from "react";
import type { ProjectInfo, SessionStats, ClientPresence } from "./api";
import type { WatcherStatus, SSEConnectionStatus } from "./hooks/useSSE";
import { WatcherStatusIndicator } from "./WatcherStatusBar";
import { KeyHint } from "./keybindings/hint/KeyHint";
import {
  PanelLeft,
  Command,
  GitBranch,
  Zap,
  DollarSign,
  Users,
  Wifi,
  WifiOff,
} from "lucide-react";

interface Props {
  project: ProjectInfo | null;
  stats: SessionStats | null;
  connectionStatus?: SSEConnectionStatus;
  sidebarOpen: boolean;
  watcherStatus: WatcherStatus | null;
  /** Connected clients for presence tracking. */
  presenceClients?: ClientPresence[];
  contextLimit: number | null;
  onToggleSidebar: () => void;
  sessionTitle?: string | null;
  showSidebarToggle?: boolean;
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
  watcherStatus,
  presenceClients,
  contextLimit,
  onToggleSidebar,
  sessionTitle,
  showSidebarToggle = false,
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
          <KeyHint label="Show sidebar" command="layout.toggleSidebar">
            <button className="status-bar-btn" onClick={onToggleSidebar} aria-label="Show sidebar">
              <PanelLeft size={13} />
            </button>
          </KeyHint>
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

      </div>

      {/* Right section */}
      <div className="status-bar-right">
        {stats && totalTokens > 0 && (
          /* Context usage is a level, so it reads as one: a meter that fills,
             with the number that matters beside it. The old three-span form
             ("336.1K / 1.0M 34%") wrapped mid-fraction and asked the reader to
             divide two abbreviated numbers to learn the one thing it was
             reporting. The exact figures live in the popover this opens. */
          <KeyHint
            command="chat.contextWindow"
            label={contextLimit
              ? `${totalTokens.toLocaleString()} of ${contextLimit.toLocaleString()} tokens used (${contextPct}%)`
              : `${totalTokens.toLocaleString()} tokens used`}
          >
            <button
              className={`status-bar-tokens status-bar-tokens-btn ${contextColorClass}`}
              onClick={onOpenContextWindow}
            >
              <Zap size={11} className="status-bar-tokens-icon" />
              {contextPct !== null ? (
                <>
                  <span
                    className="status-bar-ctx-meter"
                    role="meter"
                    aria-valuenow={contextPct}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label="Context window used"
                  >
                    <span
                      className="status-bar-ctx-fill"
                      style={{ transform: `scaleX(${Math.min(100, contextPct) / 100})` }}
                    />
                  </span>
                  <span className="status-bar-ctx-pct">{contextPct}%</span>
                </>
              ) : (
                <span className="status-bar-ctx-pct">{formatTokens(totalTokens)}</span>
              )}
            </button>
          </KeyHint>
        )}
        {stats && stats.cost > 0 && (
          <span className="status-bar-cost">
            <DollarSign size={11} />
            {stats.cost.toFixed(4)}
          </span>
        )}

        <KeyHint label="Command palette" command="palette.commands">
          <button className="status-bar-btn" onClick={onOpenCommandPalette} aria-label="Command palette">
            <Command size={13} />
          </button>
        </KeyHint>
      </div>
    </div>
  );
});
