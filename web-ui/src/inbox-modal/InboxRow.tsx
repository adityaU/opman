import React, { useEffect, useState } from "react";
import { BellRing, ChevronRight } from "lucide-react";
import type { ActivityEvent } from "../api";
import { computeMissionHandoff, computeSessionHandoff } from "../api";
import type { InboxItem, HandoffBrief } from "../api/intelligence";
import type { PermissionRequest, QuestionRequest } from "../types";
import { toPermissionInputs, toQuestionInputs } from "../hooks/intelligenceAdapters";
import { formatSource, formatTime } from "./helpers";

interface InboxRowProps {
  item: InboxItem;
  onPermissionReply: (requestId: string, reply: "once" | "always" | "reject") => Promise<void>;
  onQuestionReply: (requestId: string, question: QuestionRequest) => Promise<void>;
  onOpenMissions: () => void;
  onOpenActivityFeed: () => void;
  onOpenWatcher: () => void;
  onDismissSignal: (id: string) => void;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  activityEvents: ActivityEvent[];
}

export function InboxRow({
  item,
  onPermissionReply,
  onQuestionReply,
  onOpenMissions,
  onOpenActivityFeed,
  onOpenWatcher,
  onDismissSignal,
  permissions,
  questions,
}: InboxRowProps) {
  const [pending, setPending] = useState(false);
  const [handoff, setHandoff] = useState<HandoffBrief | null>(null);

  // ── Fetch handoff from backend ──
  useEffect(() => {
    const permInputs = toPermissionInputs(permissions);
    const qInputs = toQuestionInputs(questions);

    if (item.mission_id) {
      computeMissionHandoff({
        mission_id: item.mission_id,
        permissions: permInputs,
        questions: qInputs,
      }).then(setHandoff).catch(() => {});
    } else if (item.session_id) {
      computeSessionHandoff({
        session_id: item.session_id,
        permissions: permInputs,
        questions: qInputs,
      }).then(setHandoff).catch(() => {});
    }
  }, [item.mission_id, item.session_id, permissions, questions]);

  const priorityLabel = item.priority === "high" ? "High" : item.priority === "medium" ? "Medium" : "Low";

  // Find the original permission/question for action buttons
  const matchedPermission = item.source === "permission"
    ? permissions.find((p) => item.id === `permission:${p.id}`)
    : undefined;
  const matchedQuestion = item.source === "question"
    ? questions.find((q) => item.id === `question:${q.id}`)
    : undefined;

  const isSignal = item.source === "completion" || item.source === "watcher";

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
          <span>{formatTime(item.created_at)}</span>
        </div>
        {handoff && (
          <div className="assistant-inbox-handoff">
            <span className="assistant-inbox-handoff-title">Resume</span>
            <span className="assistant-inbox-handoff-next">{handoff.next_action}</span>
            {handoff.links.length > 0 && (
              <div className="assistant-inbox-handoff-links">
                {handoff.links.map((link) => (
                  <button
                    key={`${link.kind}:${link.source_id ?? link.label}`}
                    className="assistant-inbox-handoff-link"
                    onClick={() => {
                      if (link.kind === "mission") { onOpenMissions(); return; }
                      if (link.kind === "permission" || link.kind === "question") { onOpenMissions(); return; }
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
        {item.source === "permission" && matchedPermission && (
          <>
            <button disabled={pending} onClick={() => run(() => onPermissionReply(matchedPermission.id, "once"))}>
              Allow once
            </button>
            <button disabled={pending} onClick={() => run(() => onPermissionReply(matchedPermission.id, "reject"))}>
              Reject
            </button>
          </>
        )}
        {item.source === "question" && matchedQuestion && (
          <button disabled={pending} onClick={() => run(() => onQuestionReply(matchedQuestion.id, matchedQuestion))}>
            Answer
          </button>
        )}
        {item.source === "mission" && (
          <button onClick={onOpenMissions}>Open mission</button>
        )}
        {isSignal && (
          <button onClick={item.source === "watcher" ? onOpenWatcher : onOpenActivityFeed}>
            Open source
          </button>
        )}
        {isSignal && (
          <button className="assistant-inbox-dismiss" onClick={() => onDismissSignal(item.id)}>
            Dismiss
          </button>
        )}
        {!isSignal && item.source !== "permission" && item.source !== "question" && item.source !== "mission" && (
          <button onClick={onOpenActivityFeed}>
            <BellRing size={14} />
            <ChevronRight size={14} />
          </button>
        )}
      </div>
    </div>
  );
}
