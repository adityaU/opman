import React, { useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Bot, Brain, BriefcaseBusiness, Clock3, Inbox, Layers, Target, X } from "lucide-react";
import type { AutonomyMode } from "./api";
import {
  computeRecommendations, computeAssistantStats,
} from "./api";
import type {
  AssistantRecommendation, AssistantCenterStats, ResumeBriefing,
} from "./api/intelligence";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { AssistantSignal } from "./hooks/useAssistantState";
import { toPermissionInputs, toQuestionInputs } from "./hooks/intelligenceAdapters";

interface Props {
  onClose: () => void;
  autonomyMode: AutonomyMode;
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  assistantSignals: AssistantSignal[];
  resumeBriefing?: ResumeBriefing | null;
  latestDailySummary?: string | null;
  onQuickSetupDailySummary?: () => void;
  onQuickUpgradeAutonomy?: () => void;
  onQuickSetupDailyCopilot?: () => void;
  onOpenInbox: () => void;
  onOpenMissions: () => void;
  onOpenMemory: () => void;
  onOpenAutonomy: () => void;
  onOpenRoutines: () => void;
  onOpenDelegation: () => void;
  onOpenWorkspaces: () => void;
}

export function AssistantCenterModal({
  onClose,
  autonomyMode,
  permissions,
  questions,
  assistantSignals,
  resumeBriefing,
  latestDailySummary,
  onQuickSetupDailySummary,
  onQuickUpgradeAutonomy,
  onQuickSetupDailyCopilot,
  onOpenInbox,
  onOpenMissions,
  onOpenMemory,
  onOpenAutonomy,
  onOpenRoutines,
  onOpenDelegation,
  onOpenWorkspaces,
}: Props) {
  useEscape(onClose);

  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);

  // ── Backend-computed stats ──
  const [stats, setStats] = useState<AssistantCenterStats | null>(null);
  useEffect(() => {
    computeAssistantStats({
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
    })
      .then(setStats)
      .catch(() => {});
  }, [permissions, questions]);

  // ── Backend-computed recommendations ──
  const [recommendations, setRecommendations] = useState<AssistantRecommendation[]>([]);
  useEffect(() => {
    computeRecommendations({
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
    })
      .then((resp) => setRecommendations(resp.recommendations))
      .catch(() => {});
  }, [permissions, questions]);

  const inboxNeedsYou = stats
    ? stats.pending_permissions + stats.pending_questions + stats.blocked_missions
    : permissions.length + questions.length;

  const runRecommendation = (recommendation: AssistantRecommendation) => {
    switch (recommendation.action) {
      case "open_inbox":
        onOpenInbox();
        break;
      case "open_missions":
        onOpenMissions();
        break;
      case "open_memory":
        onOpenMemory();
        break;
      case "open_routines":
        onOpenRoutines();
        break;
      case "open_delegation":
        onOpenDelegation();
        break;
      case "open_workspaces":
        onOpenWorkspaces();
        break;
      case "open_autonomy":
        onOpenAutonomy();
        break;
      case "setup_daily_summary":
        onQuickSetupDailySummary?.();
        break;
      case "upgrade_autonomy_nudge":
        onQuickUpgradeAutonomy?.();
        break;
      case "setup_daily_copilot":
        onQuickSetupDailyCopilot?.();
        break;
    }
  };

  return (
    <div className="assistant-center-overlay" onClick={onClose}>
      <div ref={modalRef} className="assistant-center-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="assistant-center-header">
          <div className="assistant-center-header-left">
            <Bot size={16} />
            <h3>Assistant Center</h3>
          </div>
          <button onClick={onClose} aria-label="Close assistant center">
            <X size={16} />
          </button>
        </div>

        <div className="assistant-center-hero">
          <div className="assistant-center-mode">Mode: {formatMode(autonomyMode)}</div>
          <div className="assistant-center-summary">
            {inboxNeedsYou} needs attention
            {stats ? ` • ${stats.active_missions} active missions` : ""}
            {" • "}{assistantSignals.length} recent signals
          </div>
        </div>

        {resumeBriefing && (
          <div className="assistant-center-briefing">
            <div className="assistant-center-briefing-title">Back again</div>
            <div className="assistant-center-briefing-summary">{resumeBriefing.summary}</div>
            <div className="assistant-center-briefing-next">Next: {resumeBriefing.next_action}</div>
          </div>
        )}

        {latestDailySummary && (
          <div className="assistant-center-daily-summary">
            <div className="assistant-center-briefing-title">Daily summary</div>
            <div className="assistant-center-briefing-summary">{latestDailySummary}</div>
          </div>
        )}

        {recommendations.length > 0 && (
          <div className="assistant-center-recommendations">
            <div className="assistant-center-briefing-title">Recommended next</div>
            {recommendations.map((recommendation) => (
              <button
                key={recommendation.id}
                className={`assistant-center-recommendation assistant-center-recommendation-${recommendation.priority}`}
                onClick={() => runRecommendation(recommendation)}
              >
                <span className="assistant-center-recommendation-title">{recommendation.title}</span>
                <span className="assistant-center-recommendation-desc">{recommendation.rationale}</span>
              </button>
            ))}
          </div>
        )}

        <div className="assistant-center-grid">
          <AssistantCenterCard
            icon={<Inbox size={16} />}
            title="Inbox"
            value={`${inboxNeedsYou}`}
            description="Permissions, questions, and blocked work"
            onClick={onOpenInbox}
          />
          <AssistantCenterCard
            icon={<Target size={16} />}
            title="Missions"
            value={stats ? `${stats.total_missions}` : "..."}
            description={stats ? `${stats.blocked_missions} blocked, ${stats.active_missions} active` : "Loading..."}
            onClick={onOpenMissions}
          />
          <AssistantCenterCard
            icon={<Brain size={16} />}
            title="Memory"
            value={stats ? `${stats.memory_items}` : "..."}
            description="Persistent preferences and working norms"
            onClick={onOpenMemory}
          />
          <AssistantCenterCard
            icon={<Clock3 size={16} />}
            title="Routines"
            value={stats ? `${stats.active_routines}` : "..."}
            description="Scheduled and event-driven routines"
            onClick={onOpenRoutines}
          />
          <AssistantCenterCard
            icon={<BriefcaseBusiness size={16} />}
            title="Delegation"
            value={stats ? `${stats.active_delegations}` : "..."}
            description="Active delegated work items"
            onClick={onOpenDelegation}
          />
          <AssistantCenterCard
            icon={<Layers size={16} />}
            title="Workspaces"
            value={stats ? `${stats.workspace_count}` : "..."}
            description="Intent-oriented workspace launches"
            onClick={onOpenWorkspaces}
          />
        </div>

        <div className="assistant-center-footer-actions">
          <button onClick={onOpenAutonomy}>Adjust autonomy</button>
          <button onClick={onOpenInbox}>Open needs-you queue</button>
        </div>
      </div>
    </div>
  );
}

function AssistantCenterCard({
  icon,
  title,
  value,
  description,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  value: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button className="assistant-center-card" onClick={onClick}>
      <div className="assistant-center-card-top">
        <span className="assistant-center-card-icon">{icon}</span>
        <span className="assistant-center-card-value">{value}</span>
      </div>
      <div className="assistant-center-card-title">{title}</div>
      <div className="assistant-center-card-desc">{description}</div>
    </button>
  );
}

function formatMode(mode: AutonomyMode): string {
  switch (mode) {
    case "observe":
      return "Observe";
    case "nudge":
      return "Nudge";
    case "continue":
      return "Continue";
    case "autonomous":
      return "Autonomous";
  }
}
