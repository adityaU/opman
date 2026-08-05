import React, { useRef, useEffect, useCallback, useState } from "react";
import type { PermissionRequest } from "./types";
import { ShieldAlert, Check, CheckCheck, X, ExternalLink, Folder, FileText, ListChecks, Terminal } from "lucide-react";

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
    <div className="permission-dock" role="alertdialog" aria-label="Permission requests" aria-modal="false">
      {showTabs && (
        <div className="dock-tabs dock-tabs--permission" role="tablist" aria-label="Pending permissions">
          {permissions.map((perm, idx) => (
            <button
              key={perm.id}
              className={`dock-tab dock-tab--permission ${idx === activeTab ? "dock-tab--active" : ""}`}
              onClick={() => setActiveTab(idx)}
              aria-selected={idx === activeTab}
              aria-controls={`permission-panel-${perm.id}`}
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
      if ((e.target as HTMLElement).closest("button")) return;
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
    <div className="permission-card" id={`permission-panel-${perm.id}`} role="tabpanel" onKeyDown={handleKeyDown}>
      <div className="permission-header">
        <div className="permission-header-main">
          <ShieldAlert size={17} className="permission-icon" />
          <div>
            <div className="permission-title">Permission Required</div>
            <div className="permission-subtitle">Review this action before it runs</div>
          </div>
          {isCrossSession && <span className="permission-badge-subagent">subagent</span>}
        </div>
        <div className="permission-header-meta">
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
          <span className="permission-hint">Enter = allow · A = always · Esc = reject</span>
        </div>
      </div>
      <div className="permission-body">
        <div className="permission-tool">{perm.toolName || "Unknown permission"}</div>
        {perm.description && (
          <div className="permission-desc">{perm.description}</div>
        )}
        <PermissionDetails metadata={perm.metadata} patterns={perm.patterns} />
      </div>
      <div className="permission-actions" aria-label="Permission actions">
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

const INTERNAL_PERMISSION_KEYS = new Set([
  "id", "requestID", "threadId", "threadID", "sessionID", "turnId", "turnID", "itemId", "itemID",
]);

const PERMISSION_LABELS: Record<string, string> = {
  availableDecisions: "Approval options",
  command: "Command",
  cmd: "Command",
  cwd: "Working directory",
  directory: "Working directory",
  file_path: "File",
  filePath: "File",
  path: "Path",
  notebook_path: "Notebook",
  notebookPath: "Notebook",
  reason: "Reason",
  execpolicy_amendment: "Policy change",
  execpolicyAmendment: "Policy change",
};

function humanizePermissionKey(key: string): string {
  return PERMISSION_LABELS[key] || key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .replace(/^./, (character) => character.toUpperCase());
}

function scalarPermissionValue(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function formatDecision(value: unknown): string {
  const decision = scalarPermissionValue(value);
  if (decision) {
    return {
      accept: "Allow once",
      acceptForSession: "Always allow",
      acceptWithExecpolicyAmendment: "Allow with policy update",
      decline: "Reject",
    }[decision] || humanizePermissionKey(decision);
  }
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const key = Object.keys(value)[0];
    if (key) return formatDecision(key);
  }
  return "Available option";
}

function renderPermissionValue(value: unknown, depth = 0): React.ReactNode {
  const scalar = scalarPermissionValue(value);
  if (scalar !== null) return scalar;
  if (value === null || value === undefined) return null;
  if (depth > 1) return "Details available";
  if (Array.isArray(value)) {
    return (
      <div className="permission-value-list">
        {value.slice(0, 8).map((item, index) => (
          <span className="permission-value-chip" key={`${index}-${scalarPermissionValue(item) || "item"}`}>
            {renderPermissionValue(item, depth + 1)}
          </span>
        ))}
        {value.length > 8 && <span className="permission-value-more">+{value.length - 8} more</span>}
      </div>
    );
  }
  if (typeof value === "object") {
    return (
      <div className="permission-value-object">
        {Object.entries(value as Record<string, unknown>)
          .filter(([key, item]) => !INTERNAL_PERMISSION_KEYS.has(key) && item !== null && item !== undefined)
          .slice(0, 8)
          .map(([key, item]) => (
            <div className="permission-value-row" key={key}>
              <span>{humanizePermissionKey(key)}</span>
              <strong>{renderPermissionValue(item, depth + 1)}</strong>
            </div>
          ))}
      </div>
    );
  }
  return null;
}

function PermissionDetails({ metadata, patterns }: { metadata?: Record<string, unknown>; patterns?: string[] }) {
  const entries = Object.entries(metadata || {}).filter(([key, value]) => {
    if (INTERNAL_PERMISSION_KEYS.has(key) || value === null || value === undefined) return false;
    if (Array.isArray(value) && value.length === 0) return false;
    return true;
  });
  const decisions = metadata?.availableDecisions;
  const decisionList = Array.isArray(decisions) ? decisions : [];
  const preferredKeys = ["command", "cmd", "cwd", "directory", "file_path", "filePath", "path", "notebook_path", "notebookPath", "reason"];
  const visibleEntries = entries
    .filter(([key]) => key !== "availableDecisions" && !preferredKeys.includes(key))
    .slice(0, 5);

  if (!patterns?.length && !entries.length) return null;

  return (
    <div className="permission-details">
      {preferredKeys.map((key) => {
        const value = metadata?.[key];
        if (value === undefined || value === null || value === "") return null;
        const isCommand = key === "command";
        return (
          <div className={`permission-detail-row${isCommand ? " permission-detail-command" : ""}`} key={key}>
            <span className="permission-detail-label">
              {key === "command" || key === "cmd" ? <Terminal size={13} /> : key.toLowerCase().includes("path") ? <FileText size={13} /> : key === "cwd" || key === "directory" ? <Folder size={13} /> : <ListChecks size={13} />}
              {humanizePermissionKey(key)}
            </span>
            <div className="permission-detail-value">{renderPermissionValue(value)}</div>
          </div>
        );
      })}
      {!!patterns?.length && (
        <div className="permission-detail-row">
          <span className="permission-detail-label"><FileText size={13} />Requested scope</span>
          <div className="permission-patterns">
            {patterns.map((pattern, index) => <code key={`${pattern}-${index}`} className="permission-pattern">{pattern}</code>)}
          </div>
        </div>
      )}
      {decisionList.length > 0 && (
        <div className="permission-detail-row">
          <span className="permission-detail-label"><ListChecks size={13} />Approval options</span>
          <div className="permission-value-list">
            {decisionList.map((decision, index) => <span className="permission-value-chip" key={`${formatDecision(decision)}-${index}`}>{formatDecision(decision)}</span>)}
          </div>
        </div>
      )}
      {visibleEntries.map(([key, value]) => (
        <div className="permission-detail-row" key={key}>
          <span className="permission-detail-label">{humanizePermissionKey(key)}</span>
          <div className="permission-detail-value">{renderPermissionValue(value)}</div>
        </div>
      ))}
    </div>
  );
}
