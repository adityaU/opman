import React from "react";
import type { PermissionRequest } from "./types";
import { ShieldAlert, Check, CheckCheck, X } from "lucide-react";

interface Props {
  permissions: PermissionRequest[];
  onReply: (requestId: string, reply: "once" | "always" | "reject") => void;
}

export function PermissionDock({ permissions, onReply }: Props) {
  return (
    <div className="permission-dock">
      {permissions.map((perm) => (
        <div key={perm.id} className="permission-card">
          <div className="permission-header">
            <ShieldAlert size={16} className="permission-icon" />
            <span className="permission-title">Permission Required</span>
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
              className="permission-btn permission-btn-allow"
              onClick={() => onReply(perm.id, "once")}
            >
              <Check size={14} />
              Allow Once
            </button>
            <button
              className="permission-btn permission-btn-always"
              onClick={() => onReply(perm.id, "always")}
            >
              <CheckCheck size={14} />
              Always Allow
            </button>
            <button
              className="permission-btn permission-btn-reject"
              onClick={() => onReply(perm.id, "reject")}
            >
              <X size={14} />
              Reject
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
