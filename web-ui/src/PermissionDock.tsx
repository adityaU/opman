import React, { useRef, useEffect, useCallback, useMemo } from "react";
import type { PermissionRequest } from "./types";
import { ShieldAlert, Check, CheckCheck, X } from "lucide-react";
import { DockCard, DockTabs, type DockTab } from "./dock/DockCard";
import { useClampedTab } from "./dock/useClampedTab";
import { PermissionDetails } from "./dock/PermissionDetails";

type Reply = "once" | "always" | "reject";

interface Props {
  permissions: PermissionRequest[];
  /** When set, permissions from other sessions show a "subagent" badge */
  activeSessionId?: string | null;
  onReply: (requestId: string, reply: Reply) => void;
  /** Navigate to a session by its ID */
  onGoToSession?: (sessionId: string) => void;
}

export const PermissionDock = React.memo(function PermissionDock({
  permissions,
  activeSessionId,
  onReply,
  onGoToSession,
}: Props) {
  const [activeTab, setActiveTab] = useClampedTab(permissions.length);

  const tabs = useMemo<DockTab[]>(
    () =>
      permissions.map((request, index) => ({
        id: request.id,
        label: request.toolName || `Permission ${index + 1}`,
        icon: <ShieldAlert size={12} />,
        badge: !!activeSessionId && request.sessionID !== activeSessionId ? "sub" : undefined,
      })),
    [permissions, activeSessionId],
  );

  if (permissions.length === 0) return null;
  const active = permissions[activeTab];
  if (!active) return null;

  return (
    <div className="dock-panel dock-panel--permission" role="alertdialog" aria-label="Permission requests" aria-modal="false">
      <DockTabs tabs={tabs} active={activeTab} onSelect={setActiveTab} kind="permission" label="Pending permissions" />
      <PermissionCard
        key={active.id}
        perm={active}
        isCrossSession={!!activeSessionId && active.sessionID !== activeSessionId}
        onReply={onReply}
        onGoToSession={onGoToSession}
      />
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
  onReply: (requestId: string, reply: Reply) => void;
  onGoToSession?: (sessionId: string) => void;
}) {
  const allowOnceRef = useRef<HTMLButtonElement>(null);

  // Auto-focus "Allow Once" so Enter answers the card without a click.
  useEffect(() => {
    const timer = setTimeout(() => allowOnceRef.current?.focus(), 50);
    return () => clearTimeout(timer);
  }, [perm.id]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if ((event.target as HTMLElement).closest("button")) return;
      const reply: Reply | null =
        event.key === "Enter" ? "once"
        : event.key === "a" || event.key === "A" ? "always"
        : event.key === "Escape" || event.key === "r" || event.key === "R" ? "reject"
        : null;
      if (!reply) return;
      event.preventDefault();
      onReply(perm.id, reply);
    },
    [perm.id, onReply],
  );

  const footer = (
    <>
      <button
        type="button"
        ref={allowOnceRef}
        className="dock-btn dock-btn--allow"
        onClick={() => onReply(perm.id, "once")}
        aria-label="Allow once"
      >
        <Check size={14} />
        Allow Once
      </button>
      <button
        type="button"
        className="dock-btn dock-btn--always"
        onClick={() => onReply(perm.id, "always")}
        aria-label="Always allow"
      >
        <CheckCheck size={14} />
        Always Allow
      </button>
      <button
        type="button"
        className="dock-btn dock-btn--reject"
        onClick={() => onReply(perm.id, "reject")}
        aria-label="Reject"
      >
        <X size={14} />
        Reject
      </button>
    </>
  );

  return (
    <DockCard
      kind="permission"
      icon={<ShieldAlert size={16} />}
      title="Permission Required"
      subtitle="Review this action before it runs"
      isCrossSession={isCrossSession}
      sessionId={perm.sessionID}
      onGoToSession={onGoToSession}
      hint="Enter = allow · A = always · Esc = reject"
      footer={footer}
      onKeyDown={handleKeyDown}
    >
      <div className="permission-tool">{perm.toolName || "Unknown permission"}</div>
      {perm.description && <div className="permission-desc">{perm.description}</div>}
      <PermissionDetails metadata={perm.metadata} patterns={perm.patterns} />
    </DockCard>
  );
}
