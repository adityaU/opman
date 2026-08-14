import React from "react";
import { FileText, Folder, ListChecks, Terminal } from "lucide-react";

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

export function PermissionDetails({ metadata, patterns }: { metadata?: Record<string, unknown>; patterns?: string[] }) {
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
