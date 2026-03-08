import React, { useMemo, useRef } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Bot, Brain, BriefcaseBusiness, Clock3, Inbox, Layers, Target, X } from "lucide-react";
import type {
  AutonomyMode,
  DelegatedWorkItem,
  Mission,
  PersonalMemoryItem,
  RoutineDefinition,
  WorkspaceSnapshot,
} from "./api";
import type { AssistantSignal } from "./inbox";
import type { PermissionRequest, QuestionRequest } from "./types";
import type { ResumeBriefing } from "./resumeBriefing";
import { buildRecommendations, type AssistantRecommendation } from "./recommendations";

interface Props {
  onClose: () => void;
  autonomyMode: AutonomyMode;
  missions: Mission[];
  routines: RoutineDefinition[];
  delegatedWork: DelegatedWorkItem[];
  memoryItems: PersonalMemoryItem[];
  assistantSignals: AssistantSignal[];
  permissions: PermissionRequest[];
  questions: QuestionRequest[];
  workspaces: WorkspaceSnapshot[];
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
  missions,
  routines,
  delegatedWork,
  memoryItems,
  assistantSignals,
  permissions,
  questions,
  workspaces,
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

  const stats = useMemo(() => {
    const blockedMissions = missions.filter((item) => item.status === "blocked").length;
    const activeMissions = missions.filter((item) => item.status === "active").length;
    const activeDelegation = delegatedWork.filter((item) => item.status !== "completed").length;
    const recipes = workspaces.filter((item) => item.is_recipe).length;
    const inboxNeedsYou = permissions.length + questions.length + blockedMissions;
    const scheduledRoutines = routines.filter((item) => item.trigger === "daily_summary").length;
    return {
      blockedMissions,
      activeMissions,
      activeDelegation,
      recipes,
      inboxNeedsYou,
      scheduledRoutines,
    };
  }, [missions, delegatedWork, workspaces, permissions, questions]);

  const recommendations = useMemo(
    () =>
      buildRecommendations({
        autonomyMode,
        missions,
        delegatedWork,
        memoryItems,
        routines,
        permissions,
        questions,
        workspaces,
      }),
    [autonomyMode, missions, delegatedWork, memoryItems, routines, permissions, questions, workspaces]
  );

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
            {stats.inboxNeedsYou} needs attention • {stats.activeMissions} active missions • {assistantSignals.length} recent signals
          </div>
        </div>

        {resumeBriefing && (
          <div className="assistant-center-briefing">
            <div className="assistant-center-briefing-title">Back again</div>
            <div className="assistant-center-briefing-summary">{resumeBriefing.summary}</div>
            <div className="assistant-center-briefing-next">Next: {resumeBriefing.nextAction}</div>
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
            value={`${stats.inboxNeedsYou}`}
            description="Permissions, questions, and blocked work"
            onClick={onOpenInbox}
          />
          <AssistantCenterCard
            icon={<Target size={16} />}
            title="Missions"
            value={`${missions.length}`}
            description={`${stats.blockedMissions} blocked, ${stats.activeMissions} active`}
            onClick={onOpenMissions}
          />
          <AssistantCenterCard
            icon={<Brain size={16} />}
            title="Memory"
            value={`${memoryItems.length}`}
            description="Persistent preferences and working norms"
            onClick={onOpenMemory}
          />
          <AssistantCenterCard
            icon={<Clock3 size={16} />}
            title="Routines"
            value={`${routines.length}`}
            description={`${stats.scheduledRoutines} scheduled, ${routines.length - stats.scheduledRoutines} event-driven`}
            onClick={onOpenRoutines}
          />
          <AssistantCenterCard
            icon={<BriefcaseBusiness size={16} />}
            title="Delegation"
            value={`${delegatedWork.length}`}
            description={`${stats.activeDelegation} not completed`}
            onClick={onOpenDelegation}
          />
          <AssistantCenterCard
            icon={<Layers size={16} />}
            title="Recipes"
            value={`${stats.recipes}`}
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
