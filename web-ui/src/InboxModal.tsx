import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { BellRing, ChevronRight, Inbox, X } from "lucide-react";
import { fetchMissions, replyPermission, replyQuestion } from "./api";
import type { Mission, PersonalMemoryItem } from "./api";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { WatcherStatus } from "./hooks/useSSE";
import { buildInboxItems } from "./inbox";
import type { AssistantSignal, InboxItem } from "./inbox";
import { buildMissionHandoff, buildSessionHandoff } from "./handoffs";
import type { ActivityEvent } from "./api";

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
  const [missions, setMissions] = useState<Mission[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<FilterMode>("all");
  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  useEffect(() => {
    let cancelled = false;
    fetchMissions()
      .then((resp) => {
        if (!cancelled) setMissions(resp.missions);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const items = useMemo(
    () =>
      buildInboxItems({
        permissions,
        questions,
        missions,
        watcherStatus,
        signals,
      }),
    [permissions, questions, missions, watcherStatus, signals]
  );

  const filtered = useMemo(() => {
    if (filter === "unresolved") {
      return items.filter((item) => item.state === "unresolved");
    }
    if (filter === "high") {
      return items.filter((item) => item.priority === "high");
    }
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
                missions={missions}
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

function InboxRow({
  item,
  onPermissionReply,
  onQuestionReply,
  onOpenMissions,
  onOpenActivityFeed,
  onOpenWatcher,
  onDismissSignal,
  missions,
  permissions,
  questions,
  activityEvents,
}: {
  item: InboxItem;
  onPermissionReply: (requestId: string, reply: "once" | "always" | "reject") => Promise<void>;
  onQuestionReply: (requestId: string, question: QuestionRequest) => Promise<void>;
  onOpenMissions: () => void;
  onOpenActivityFeed: () => void;
  onOpenWatcher: () => void;
  onDismissSignal: (id: string) => void;
  missions: Mission[];
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
}) {
  const [pending, setPending] = useState(false);

  const priorityLabel = item.priority === "high" ? "High" : item.priority === "medium" ? "Medium" : "Low";
  const mission = item.missionId ? missions.find((entry) => entry.id === item.missionId) : undefined;
  const handoff = mission
    ? buildMissionHandoff({
        mission,
        permissions,
        questions,
        activityEvents,
      })
    : buildSessionHandoff({
        sessionId: item.sessionId ?? null,
        permissions,
        questions,
        activityEvents,
      });

  const run = async (fn: () => Promise<void>) => {
    setPending(true);
    try {
      await fn();
    } finally {
      setPending(false);
    }
  };

  return (
    <div className={`assistant-inbox-item assistant-inbox-item-${item.priority}`}>
      <div className="assistant-inbox-item-main">
        <div className="assistant-inbox-item-row">
          <span className="assistant-inbox-item-title">{item.title}</span>
          <span className={`assistant-inbox-priority assistant-inbox-priority-${item.priority}`}>
            {priorityLabel}
          </span>
        </div>
        <div className="assistant-inbox-item-desc">{item.description}</div>
        <div className="assistant-inbox-item-meta">
          <span>{formatSource(item.source)}</span>
          <span>{formatTime(item.createdAt)}</span>
        </div>
        {handoff && (
          <div className="assistant-inbox-handoff">
            <span className="assistant-inbox-handoff-title">Resume</span>
            <span className="assistant-inbox-handoff-next">{handoff.nextAction}</span>
            {handoff.links.length > 0 && (
              <div className="assistant-inbox-handoff-links">
                {handoff.links.map((link) => (
                  <button
                    key={`${link.kind}:${link.sourceId ?? link.label}`}
                    className="assistant-inbox-handoff-link"
                    onClick={() => {
                      if (link.kind === "mission") {
                        onOpenMissions();
                        return;
                      }
                      if (link.kind === "permission" || link.kind === "question") {
                        onOpenMissions();
                        return;
                      }
                      onOpenActivityFeed();
                    }}
                  >
                    {link.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
      <div className="assistant-inbox-item-actions">
        {item.source === "permission" && item.permission && (
          <>
            <button disabled={pending} onClick={() => run(() => onPermissionReply(item.permission!.id, "once"))}>
              Allow once
            </button>
            <button disabled={pending} onClick={() => run(() => onPermissionReply(item.permission!.id, "reject"))}>
              Reject
            </button>
          </>
        )}
        {item.source === "question" && item.question && (
          <button disabled={pending} onClick={() => run(() => onQuestionReply(item.question!.id, item.question!))}>
            Answer
          </button>
        )}
        {item.source === "mission" && (
          <button onClick={onOpenMissions}>
            Open mission
          </button>
        )}
        {(item.source === "completion" || item.source === "watcher") && (
          <button onClick={item.source === "watcher" ? onOpenWatcher : onOpenActivityFeed}>
            Open source
          </button>
        )}
        {item.signal && (
          <button className="assistant-inbox-dismiss" onClick={() => onDismissSignal(item.signal!.id)}>
            Dismiss
          </button>
        )}
        {!item.signal && item.source !== "permission" && item.source !== "question" && item.source !== "mission" && (
          <button onClick={onOpenActivityFeed}>
            <BellRing size={14} />
            <ChevronRight size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

function formatSource(source: InboxItem["source"]): string {
  switch (source) {
    case "permission":
      return "Permission";
    case "question":
      return "Question";
    case "mission":
      return "Blocked mission";
    case "watcher":
      return "Watcher";
    case "completion":
      return "Completion";
  }
}

function formatTime(time: number): string {
  return new Date(time).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}
