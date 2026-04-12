import React, { useRef, useEffect, useCallback, useState } from "react";
import type { PermissionRequest } from "./types";
import { ShieldAlert, Check, CheckCheck, X, ExternalLink } from "lucide-react";

interface Props {
  permissions: PermissionRequest[];
  /** When set, permissions from other sessions show a "subagent" badge */
  activeSessionId?: string | null;
  onReply: (requestId: string, reply: "once" | "always" | "reject") => void;
  /** Navigate to a session by its ID */
  onGoToSession?: (sessionId: string) => void;
}

export const PermissionDock = React.memo(function PermissionDock({ permissions, activeSessionId, onReply, onGoToSession }: Props) {
  const [activeTab, setActiveTab] = useState(0);

  // Clamp activeTab when permissions list changes
  useEffect(() => {
    if (activeTab >= permissions.length) {
      setActiveTab(Math.max(0, permissions.length - 1));
    }
  }, [permissions.length, activeTab]);

  if (permissions.length === 0) return null;

  const showTabs = permissions.length > 1;
  const activePerm = permissions[Math.min(activeTab, permissions.length - 1)];

  return (
    <div className="permission-dock" role="alertdialog" aria-label="Permission requests">
      {showTabs && (
        <div className="dock-tabs dock-tabs--permission">
          {permissions.map((perm, idx) => (
            <button
              key={perm.id}
              className={`dock-tab dock-tab--permission ${idx === activeTab ? "dock-tab--active" : ""}`}
              onClick={() => setActiveTab(idx)}
              aria-selected={idx === activeTab}
              role="tab"
            >
              <ShieldAlert size={12} />
              <span className="dock-tab-label">
                {perm.toolName || `Permission ${idx + 1}`}
              </span>
              {!!activeSessionId && perm.sessionID !== activeSessionId && (
                <span className="dock-tab-badge">sub</span>
              )}
            </button>
          ))}
        </div>
      )}
      {activePerm && (
        <PermissionCard
          key={activePerm.id}
          perm={activePerm}
          isCrossSession={!!activeSessionId && activePerm.sessionID !== activeSessionId}
          onReply={onReply}
          onGoToSession={onGoToSession}
        />
      )}
    </div>
  );
});

function PermissionCard({
  perm,
  isCrossSession,
  onReply,
  onGoToSession,
}: {
  perm: PermissionRequest;
  isCrossSession: boolean;
  onReply: (requestId: string, reply: "once" | "always" | "reject") => void;
  onGoToSession?: (sessionId: string) => void;
}) {
  const allowOnceRef = useRef<HTMLButtonElement>(null);

  // Auto-focus the "Allow Once" button when the card mounts
  useEffect(() => {
    const timer = setTimeout(() => {
      allowOnceRef.current?.focus();
    }, 50);
    return () => clearTimeout(timer);
  }, [perm.id]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        onReply(perm.id, "once");
      } else if (e.key === "a" || e.key === "A") {
        e.preventDefault();
        onReply(perm.id, "always");
      } else if (e.key === "Escape" || e.key === "r" || e.key === "R") {
        e.preventDefault();
        onReply(perm.id, "reject");
      }
    },
    [perm.id, onReply]
  );

  return (
    <div className="permission-card" onKeyDown={handleKeyDown}>
      <div className="permission-header">
        <ShieldAlert size={16} className="permission-icon" />
        <span className="permission-title">Permission Required</span>
        {isCrossSession && <span className="permission-badge-subagent">subagent</span>}
        {perm.sessionID && onGoToSession && (
          <button
            className="dock-session-link"
            onClick={(e) => { e.stopPropagation(); onGoToSession(perm.sessionID); }}
            title={`Go to session ${perm.sessionID.slice(0, 8)}`}
            aria-label="Go to session"
          >
            <ExternalLink size={11} />
            <span>{perm.sessionID.slice(0, 8)}</span>
          </button>
        )}
        <span className="permission-hint">Enter = allow &middot; A = always &middot; Esc = reject</span>
      </div>
      <div className="permission-body">
        <div className="permission-tool">{perm.toolName || "Unknown permission"}</div>
        {perm.description && (
          <div className="permission-desc">{perm.description}</div>
        )}
        {perm.patterns && perm.patterns.length > 0 && (
          <div className="permission-patterns">
            {perm.patterns.map((p, i) => (
              <code key={i} className="permission-pattern">{p}</code>
            ))}
          </div>
        )}
        {perm.metadata && Object.keys(perm.metadata).length > 0 && (
          <pre className="permission-args">
            {JSON.stringify(perm.metadata, null, 2)}
          </pre>
        )}
      </div>
      <div className="permission-actions">
        <button
          ref={allowOnceRef}
          className="permission-btn permission-btn-allow"
          onClick={() => onReply(perm.id, "once")}
          aria-label="Allow once"
        >
          <Check size={14} />
          Allow Once
        </button>
        <button
          className="permission-btn permission-btn-always"
          onClick={() => onReply(perm.id, "always")}
          aria-label="Always allow"
        >
          <CheckCheck size={14} />
          Always Allow
        </button>
        <button
          className="permission-btn permission-btn-reject"
          onClick={() => onReply(perm.id, "reject")}
          aria-label="Reject"
        >
          <X size={14} />
          Reject
        </button>
      </div>
    </div>
  );
}
