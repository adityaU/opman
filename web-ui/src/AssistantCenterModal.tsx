import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import { Bot, Brain, BriefcaseBusiness, Clock3, Inbox, Layers, Play, Plus, Send, Target, X } from "lucide-react";
import type { AutonomyMode } from "./api";
import {
  computeRecommendations, computeAssistantStats,
  fetchRoutines, runRoutine,
} from "./api";
import type {
  AssistantRecommendation, AssistantCenterStats, ResumeBriefing,
} from "./api/intelligence";
import type { RoutineDefinition, RoutineRunRecord } from "./api/workflows";
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
      .catch((e) => console.warn("[AssistantCenter] Failed to compute stats:", e));
  }, [permissions, questions]);

  // ── Backend-computed recommendations ──
  const [recommendations, setRecommendations] = useState<AssistantRecommendation[]>([]);
  useEffect(() => {
    computeRecommendations({
      permissions: toPermissionInputs(permissions),
      questions: toQuestionInputs(questions),
    })
      .then((resp) => setRecommendations(resp.recommendations))
      .catch((e) => console.warn("[AssistantCenter] Failed to compute recommendations:", e));
  }, [permissions, questions]);

  // ── Quick-run routines ──
  const [routines, setRoutines] = useState<RoutineDefinition[]>([]);
  const [runningRoutineId, setRunningRoutineId] = useState<string | null>(null);
  useEffect(() => {
    fetchRoutines()
      .then((resp) => setRoutines(resp.routines.filter((r) => r.enabled)))
      .catch((e) => console.warn("[AssistantCenter] Failed to fetch routines:", e));
    const handler = () => {
      fetchRoutines()
        .then((resp) => setRoutines(resp.routines.filter((r) => r.enabled)))
        .catch((e) => console.warn("[AssistantCenter] Failed to refresh routines:", e));
    };
    window.addEventListener("opman:routine-updated", handler);
    return () => window.removeEventListener("opman:routine-updated", handler);
  }, []);

  const [quickRunError, setQuickRunError] = useState<string | null>(null);

  const handleQuickRun = useCallback(async (routineId: string) => {
    setRunningRoutineId(routineId);
    setQuickRunError(null);
    try {
      await runRoutine(routineId);
    } catch (e: any) {
      setQuickRunError(e?.message || "Failed to run routine");
    } finally {
      setRunningRoutineId(null);
    }
  }, []);

  const inboxNeedsYou = stats
    ? stats.pending_permissions + stats.pending_questions + stats.paused_missions
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
            description={stats ? `${stats.paused_missions} paused, ${stats.active_missions} active` : "Loading..."}
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

        {routines.length > 0 && (
          <div className="assistant-center-quick-routines">
            <div className="assistant-center-briefing-title" style={{ display: "flex", alignItems: "center", gap: "6px" }}>
              Routines
              <span style={{
                fontSize: "11px",
                fontWeight: 500,
                background: "var(--color-muted, rgba(128,128,128,0.15))",
                color: "var(--color-text)",
                borderRadius: "8px",
                padding: "1px 7px",
                lineHeight: "16px",
              }}>
                {routines.length}
              </span>
            </div>
            {quickRunError && (
              <div className="assistant-center-routine-error">
                {quickRunError}
                <button className="assistant-center-routine-error-dismiss" onClick={() => setQuickRunError(null)}>&times;</button>
              </div>
            )}
            <div className="assistant-center-routine-list">
              {routines.slice(0, 8).map((routine) => (
                <button
                  key={routine.id}
                  className="assistant-center-routine-btn"
                  onClick={() => handleQuickRun(routine.id)}
                  disabled={runningRoutineId === routine.id}
                  title={routine.prompt ? `Prompt: ${routine.prompt.slice(0, 100)}` : routine.name}
                >
                  <span className="assistant-center-routine-icon">
                    {routine.action === "send_message" ? <Send size={12} /> : <Play size={12} />}
                  </span>
                  <span className="assistant-center-routine-name">{routine.name}</span>
                  <span style={{
                    fontSize: "10px",
                    fontWeight: 500,
                    padding: "1px 5px",
                    borderRadius: "4px",
                    background: routine.trigger === "scheduled"
                      ? "rgba(59,130,246,0.12)"
                      : "rgba(128,128,128,0.12)",
                    color: routine.trigger === "scheduled"
                      ? "var(--color-info, #3b82f6)"
                      : "var(--color-muted, #888)",
                    whiteSpace: "nowrap",
                    flexShrink: 0,
                  }}>
                    {formatTriggerLabel(routine.trigger)}
                  </span>
                  {routine.trigger === "scheduled" && routine.cron_expr && (
                    <span style={{
                      fontSize: "10px",
                      color: "var(--color-muted, #888)",
                      whiteSpace: "nowrap",
                      flexShrink: 0,
                      display: "flex",
                      alignItems: "center",
                      gap: "3px",
                    }}>
                      <Clock3 size={10} />
                      {describeCronShort(routine.cron_expr)}
                    </span>
                  )}
                  {runningRoutineId === routine.id && (
                    <span className="assistant-center-routine-running">Running...</span>
                  )}
                </button>
              ))}
              <button
                className="assistant-center-routine-btn"
                onClick={onOpenRoutines}
                style={{ justifyContent: "center", opacity: 0.7 }}
                title="Create a new routine"
              >
                <Plus size={12} />
                <span className="assistant-center-routine-name">Create routine</span>
              </button>
            </div>
            {routines.length > 8 && (
              <button className="assistant-center-routine-more" onClick={onOpenRoutines}>
                +{routines.length - 8} more routines
              </button>
            )}
          </div>
        )}

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

const CRON_LABEL_MAP: Record<string, string> = {
  "0 */5 * * * *": "Every 5min",
  "0 */15 * * * *": "Every 15min",
  "0 */30 * * * *": "Every 30min",
  "0 0 * * * *": "Every hour",
  "0 0 */2 * * *": "Every 2h",
  "0 0 */6 * * *": "Every 6h",
  "0 0 9 * * *": "Daily 9 AM",
  "0 0 0 * * *": "Daily midnight",
  "0 0 9 * * 1-5": "Weekdays 9 AM",
  "0 0 9 * * 1": "Mon 9 AM",
};

function describeCronShort(expr: string): string {
  if (!expr) return "";
  if (CRON_LABEL_MAP[expr]) return CRON_LABEL_MAP[expr];
  // Fallback: show raw expression truncated
  return expr.length > 16 ? expr.slice(0, 14) + "…" : expr;
}

function formatTriggerLabel(trigger: string): string {
  if (trigger === "scheduled") return "scheduled";
  return trigger.replace(/_/g, " ");
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
