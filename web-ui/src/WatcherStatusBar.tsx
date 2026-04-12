import React, { useState, useEffect, useRef } from "react";
import type { WatcherStatus } from "./hooks/useSSE";
import { Eye } from "lucide-react";

interface Props {
  watcherStatus: WatcherStatus | null;
  onClick?: () => void;
}

/**
 * Compact watcher status indicator for the bottom status bar.
 * Shows a pulsing dot + status text when a watcher is active for the session.
 * Driven entirely by SSE `watcher_status` events — no polling.
 *
 * For the countdown state, a local 1-second timer ticks the displayed
 * `idle_since_secs` upward for smooth second-by-second updates between
 * SSE events.
 */
export function WatcherStatusIndicator({ watcherStatus, onClick }: Props) {
  // Local tick counter for smooth countdown display
  const [localIdleSecs, setLocalIdleSecs] = useState<number | null>(null);
  const tickRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    // Clear any running tick timer
    if (tickRef.current) {
      clearInterval(tickRef.current);
      tickRef.current = null;
    }

    if (!watcherStatus) {
      setLocalIdleSecs(null);
      return;
    }

    if (watcherStatus.action === "countdown" && watcherStatus.idle_since_secs != null) {
      // Start from the SSE-reported value and tick up every second
      let secs = watcherStatus.idle_since_secs;
      setLocalIdleSecs(secs);
      tickRef.current = setInterval(() => {
        secs += 1;
        setLocalIdleSecs(secs);
      }, 1000);
    } else {
      setLocalIdleSecs(watcherStatus.idle_since_secs);
    }

    return () => {
      if (tickRef.current) {
        clearInterval(tickRef.current);
        tickRef.current = null;
      }
    };
  }, [watcherStatus]);

  if (!watcherStatus) return null;

  const { action, idle_since_secs } = watcherStatus;
  const displaySecs = localIdleSecs ?? idle_since_secs ?? 0;

  let dotClass = "watcher-dot";
  let label = "";

  if (action === "countdown") {
    dotClass += " watcher-dot-green";
    label = `idle ${displaySecs}s -- continuing soon`;
  } else if (action === "triggered") {
    dotClass += " watcher-dot-yellow";
    label = "running";
  } else if (action === "created") {
    dotClass += " watcher-dot-muted";
    label = "watching";
  } else if (action === "cancelled") {
    dotClass += " watcher-dot-muted";
    label = "cancelled";
  } else {
    dotClass += " watcher-dot-muted";
    label = action;
  }

  return (
    <button
      className="watcher-status-indicator"
      onClick={onClick}
      title="Session Watcher (Cmd+Shift+W)"
    >
      <span className={dotClass} />
      <Eye size={11} />
      <span className="watcher-status-label">{label}</span>
    </button>
  );
}
