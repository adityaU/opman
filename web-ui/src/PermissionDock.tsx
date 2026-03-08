import React, { useRef, useEffect, useCallback } from "react";
import type { PermissionRequest } from "./types";
import { ShieldAlert, Check, CheckCheck, X } from "lucide-react";

interface Props {
  permissions: PermissionRequest[];
  onReply: (requestId: string, reply: "once" | "always" | "reject") => void;
}

export const PermissionDock = React.memo(function PermissionDock({ permissions, onReply }: Props) {
  return (
    <div className="permission-dock" role="alertdialog" aria-label="Permission requests">
      {permissions.map((perm) => (
        <PermissionCard key={perm.id} perm={perm} onReply={onReply} />
      ))}
    </div>
  );
});

function PermissionCard({
  perm,
  onReply,
}: {
  perm: PermissionRequest;
  onReply: (requestId: string, reply: "once" | "always" | "reject") => void;
}) {
  const allowOnceRef = useRef<HTMLButtonElement>(null);

  // Auto-focus the "Allow Once" button when the card mounts
  useEffect(() => {
    // Small delay to ensure the DOM is ready
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
        <span className="permission-hint">Enter=allow, A=always, Esc=reject</span>
      </div>
      <div className="permission-body">
        <div className="permission-tool">{perm.toolName}</div>
        {perm.description && (
          <div className="permission-desc">{perm.description}</div>
        )}
        {perm.args && Object.keys(perm.args).length > 0 && (
          <pre className="permission-args">
            {JSON.stringify(perm.args, null, 2)}
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
