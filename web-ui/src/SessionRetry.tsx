import React, { useState, useEffect, useMemo } from "react";
import type { SessionStatus } from "./hooks/sse/types";
import { Loader2, AlertTriangle } from "lucide-react";

export interface SessionRetryProps {
  sessionStatus: SessionStatus;
}

/**
 * Inline retry banner — shown at the bottom of the message timeline when the
 * active session is in retry state.  Displays: error message, attempt number,
 * and a 1-second countdown to the next retry.
 *
 * Mirrors the pattern from OpenCode's session-retry.tsx (SolidJS → React).
 */
export const SessionRetry = React.memo(function SessionRetry({
  sessionStatus,
}: SessionRetryProps) {
  const retry = sessionStatus.type === "retry" ? sessionStatus : null;
  const [seconds, setSeconds] = useState(0);

  useEffect(() => {
    if (!retry) return;
    const update = () => setSeconds(Math.max(0, Math.round((retry.next - Date.now()) / 1000)));
    update();
    const timer = setInterval(update, 1000);
    return () => clearInterval(timer);
  }, [retry]);

  const message = useMemo(() => {
    if (!retry) return "";
    const msg = retry.message;
    if (msg.length > 120) return msg.slice(0, 120) + "…";
    return msg;
  }, [retry]);

  const info = useMemo(() => {
    if (!retry) return "";
    const delay = seconds > 0 ? `in ${seconds}s` : "";
    const parts = [`Retrying`, delay].filter(Boolean).join(" ");
    return `${parts} · attempt #${retry.attempt}`;
  }, [retry, seconds]);

  if (!retry) return null;

  return (
    <div className="session-retry-banner">
      <div className="session-retry-inner">
        <Loader2 size={14} className="session-retry-spinner" />
        <div className="session-retry-content">
          <div className="session-retry-message">
            <AlertTriangle size={12} />
            <span>{message}</span>
          </div>
          <div className="session-retry-info">{info}</div>
        </div>
      </div>
    </div>
  );
});
