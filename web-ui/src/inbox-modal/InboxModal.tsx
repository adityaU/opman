import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "../hooks/useKeyboard";
import { useFocusTrap } from "../hooks/useFocusTrap";
import { Inbox, X } from "lucide-react";
import { replyPermission, replyQuestion, computeInbox } from "../api";
import type { PersonalMemoryItem } from "../api";
import type { InboxItem } from "../api/intelligence";
import type { PermissionRequest, QuestionRequest } from "../types";
import type { WatcherStatus } from "../hooks/useSSE";
import type { AssistantSignal } from "../hooks/useAssistantState";
import type { ActivityEvent } from "../api";
import { toPermissionInputs, toQuestionInputs, toSignalInputs } from "../hooks/intelligenceAdapters";
import { InboxRow } from "./InboxRow";

type FilterMode = "all" | "unresolved" | "high";

interface Props {
  onClose: () => void;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  watcherStatus: WatcherStatus | null;
  signals: AssistantSignal[];
  onDismissSignal: (id: string) => void;
  onOpenMissions: () => void;
  onOpenActivityFeed: () => void;
  onOpenWatcher: () => void;
  onPermissionResolved: (id: string) => void;
  onQuestionResolved: (id: string) => void;
  activityEvents?: ActivityEvent[];
  activeMemoryItems?: PersonalMemoryItem[];
}

export function InboxModal({
  onClose,
  permissions,
  questions,
  watcherStatus,
  signals,
  onDismissSignal,
  onOpenMissions,
  onOpenActivityFeed,
  onOpenWatcher,
  onPermissionResolved,
  onQuestionResolved,
  activityEvents = [],
  activeMemoryItems = [],
}: Props) {
  const [items, setItems] = useState<InboxItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<FilterMode>("all");
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  // ── Compute inbox from backend ──
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    computeInbox({
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
      watcher_status: watcherStatus ? {
        session_id: watcherStatus.session_id,
        action: watcherStatus.action,
      } : null,
      signals: toSignalInputs(signals),
    })
      .then((resp) => {
        if (!cancelled) setItems(resp.items);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [permissions, questions, watcherStatus, signals]);

  const filtered = useMemo(() => {
    if (filter === "unresolved") return items.filter((item) => item.state === "unresolved");
    if (filter === "high") return items.filter((item) => item.priority === "high");
    return items;
  }, [items, filter]);

  const handlePermissionReply = useCallback(
    async (requestId: string, reply: "once" | "always" | "reject") => {
      await replyPermission(requestId, reply);
      onPermissionResolved(requestId);
    },
    [onPermissionResolved]
  );

  const handleQuestionReply = useCallback(
    async (requestId: string, question: QuestionRequest) => {
      const answers = question.questions.map((entry) => {
        if (entry.type === "confirm") return ["yes"];
        if (entry.type === "select" && entry.options?.length) return [entry.options[0]];
        return ["Acknowledged"];
      });
      await replyQuestion(requestId, answers);
      onQuestionResolved(requestId);
    },
    [onQuestionResolved]
  );

  return (
    <div className="assistant-inbox-overlay" onClick={onClose}>
      <div ref={modalRef} className="assistant-inbox-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="assistant-inbox-header">
          <div className="assistant-inbox-header-left">
            <Inbox size={16} />
            <h3>Assistant Inbox</h3>
            <span className="assistant-inbox-count">{items.length}</span>
          </div>
          <button onClick={onClose} aria-label="Close inbox">
            <X size={16} />
          </button>
        </div>

        <div className="assistant-inbox-filters">
          {(["all", "unresolved", "high"] as FilterMode[]).map((mode) => (
            <button
              key={mode}
              className={`assistant-inbox-filter ${filter === mode ? "active" : ""}`}
              onClick={() => setFilter(mode)}
            >
              {mode === "all" ? "All" : mode === "unresolved" ? "Needs You" : "High Priority"}
            </button>
          ))}
        </div>

        {activeMemoryItems.length > 0 && (
          <div className="assistant-memory-strip">
            <span className="assistant-memory-strip-label">Memory in play</span>
            {activeMemoryItems.slice(0, 4).map((item) => (
              <span key={item.id} className="assistant-memory-chip">{item.label}</span>
            ))}
          </div>
        )}

        <div className="assistant-inbox-body">
          {loading ? (
            <div className="assistant-inbox-empty">Loading inbox...</div>
          ) : filtered.length === 0 ? (
            <div className="assistant-inbox-empty">Nothing needs attention right now.</div>
          ) : (
            filtered.map((item) => (
              <InboxRow
                key={item.id}
                item={item}
                onPermissionReply={handlePermissionReply}
                onQuestionReply={handleQuestionReply}
                onOpenMissions={onOpenMissions}
                onOpenActivityFeed={onOpenActivityFeed}
                onOpenWatcher={onOpenWatcher}
                onDismissSignal={onDismissSignal}
                permissions={permissions}
                questions={questions}
                activityEvents={activityEvents}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
