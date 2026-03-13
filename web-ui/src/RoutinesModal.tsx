import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useEscape } from "./hooks/useKeyboard";
import { useFocusTrap } from "./hooks/useFocusTrap";
import {
  AlertTriangle, Calendar, Check, ChevronDown, ChevronRight,
  Clock3, Pencil, Play, Plus, Power, PowerOff,
  Save, Send, Trash2, X,
} from "lucide-react";
import {
  createRoutine, deleteRoutine, fetchRoutines, runRoutine, updateRoutine,
} from "./api";
import type {
  Mission, RoutineAction, RoutineDefinition, RoutineRunRecord,
  RoutineTargetMode, RoutineTrigger,
} from "./api";
import { useProviders } from "./hooks/useProviders";

/* ── Cron presets ─────────────────────────────────────── */

const CRON_PRESETS: { label: string; cron: string }[] = [
  { label: "Every 5 minutes", cron: "0 */5 * * * *" },
  { label: "Every 15 minutes", cron: "0 */15 * * * *" },
  { label: "Every 30 minutes", cron: "0 */30 * * * *" },
  { label: "Every hour", cron: "0 0 * * * *" },
  { label: "Every 2 hours", cron: "0 0 */2 * * *" },
  { label: "Every 6 hours", cron: "0 0 */6 * * *" },
  { label: "Daily at 9 AM", cron: "0 0 9 * * *" },
  { label: "Daily at midnight", cron: "0 0 0 * * *" },
  { label: "Weekdays at 9 AM", cron: "0 0 9 * * 1-5" },
  { label: "Monday at 9 AM", cron: "0 0 9 * * 1" },
];

const TIMEZONE_OPTIONS = [
  "UTC",
  "America/New_York",
  "America/Chicago",
  "America/Denver",
  "America/Los_Angeles",
  "Europe/London",
  "Europe/Berlin",
  "Europe/Paris",
  "Asia/Tokyo",
  "Asia/Shanghai",
  "Asia/Kolkata",
  "Australia/Sydney",
  "Pacific/Auckland",
];

/* ── Visual Cron Builder helpers ─────────────────────── */

interface CronFields {
  second: string;
  minute: string;
  hour: string;
  dom: string;
  month: string;
  dow: string;
}

function parseCronFields(expr: string): CronFields {
  const parts = expr.trim().split(/\s+/);
  if (parts.length === 6) {
    return { second: parts[0], minute: parts[1], hour: parts[2], dom: parts[3], month: parts[4], dow: parts[5] };
  }
  if (parts.length === 5) {
    return { second: "0", minute: parts[0], hour: parts[1], dom: parts[2], month: parts[3], dow: parts[4] };
  }
  return { second: "0", minute: "*", hour: "*", dom: "*", month: "*", dow: "*" };
}

function assembleCron(fields: CronFields): string {
  return `${fields.second} ${fields.minute} ${fields.hour} ${fields.dom} ${fields.month} ${fields.dow}`;
}

const MINUTE_OPTIONS = [
  { label: "Every minute", value: "*" },
  { label: "Every 5 min", value: "*/5" },
  { label: "Every 10 min", value: "*/10" },
  { label: "Every 15 min", value: "*/15" },
  { label: "Every 30 min", value: "*/30" },
  { label: ":00", value: "0" },
  { label: ":05", value: "5" },
  { label: ":10", value: "10" },
  { label: ":15", value: "15" },
  { label: ":20", value: "20" },
  { label: ":30", value: "30" },
  { label: ":45", value: "45" },
];

const HOUR_OPTIONS = [
  { label: "Every hour", value: "*" },
  { label: "Every 2h", value: "*/2" },
  { label: "Every 3h", value: "*/3" },
  { label: "Every 4h", value: "*/4" },
  { label: "Every 6h", value: "*/6" },
  { label: "Every 8h", value: "*/8" },
  { label: "Every 12h", value: "*/12" },
  { label: "12 AM", value: "0" },
  { label: "1 AM", value: "1" },
  { label: "2 AM", value: "2" },
  { label: "3 AM", value: "3" },
  { label: "4 AM", value: "4" },
  { label: "5 AM", value: "5" },
  { label: "6 AM", value: "6" },
  { label: "7 AM", value: "7" },
  { label: "8 AM", value: "8" },
  { label: "9 AM", value: "9" },
  { label: "10 AM", value: "10" },
  { label: "11 AM", value: "11" },
  { label: "12 PM", value: "12" },
  { label: "1 PM", value: "13" },
  { label: "2 PM", value: "14" },
  { label: "3 PM", value: "15" },
  { label: "4 PM", value: "16" },
  { label: "5 PM", value: "17" },
  { label: "6 PM", value: "18" },
  { label: "7 PM", value: "19" },
  { label: "8 PM", value: "20" },
  { label: "9 PM", value: "21" },
  { label: "10 PM", value: "22" },
  { label: "11 PM", value: "23" },
];

const DOM_OPTIONS = [
  { label: "Every day", value: "*" },
  { label: "1st", value: "1" },
  { label: "2nd", value: "2" },
  { label: "5th", value: "5" },
  { label: "10th", value: "10" },
  { label: "15th", value: "15" },
  { label: "20th", value: "20" },
  { label: "25th", value: "25" },
  { label: "Last", value: "L" },
  { label: "1,15", value: "1,15" },
  { label: "1-7", value: "1-7" },
  { label: "8-14", value: "8-14" },
  { label: "15-21", value: "15-21" },
  { label: "22-28", value: "22-28" },
];

const MONTH_OPTIONS = [
  { label: "Every month", value: "*" },
  { label: "Jan", value: "1" },
  { label: "Feb", value: "2" },
  { label: "Mar", value: "3" },
  { label: "Apr", value: "4" },
  { label: "May", value: "5" },
  { label: "Jun", value: "6" },
  { label: "Jul", value: "7" },
  { label: "Aug", value: "8" },
  { label: "Sep", value: "9" },
  { label: "Oct", value: "10" },
  { label: "Nov", value: "11" },
  { label: "Dec", value: "12" },
  { label: "Q1", value: "1-3" },
  { label: "Q2", value: "4-6" },
  { label: "Q3", value: "7-9" },
  { label: "Q4", value: "10-12" },
  { label: "H1", value: "1-6" },
  { label: "H2", value: "7-12" },
];

const DOW_OPTIONS = [
  { label: "Every day", value: "*" },
  { label: "Mon", value: "1" },
  { label: "Tue", value: "2" },
  { label: "Wed", value: "3" },
  { label: "Thu", value: "4" },
  { label: "Fri", value: "5" },
  { label: "Sat", value: "6" },
  { label: "Sun", value: "0" },
  { label: "Weekdays", value: "1-5" },
  { label: "Weekend", value: "0,6" },
  { label: "MWF", value: "1,3,5" },
  { label: "TTh", value: "2,4" },
];

function CronFieldPicker({ label, value, options, onChange, allowCustom }: {
  label: string;
  value: string;
  options: { label: string; value: string }[];
  onChange: (val: string) => void;
  allowCustom?: boolean;
}) {
  const [showCustom, setShowCustom] = useState(false);
  const isCustom = !options.some((o) => o.value === value);

  return (
    <div className="cron-field">
      <div className="cron-field-label">{label}</div>
      <div className="cron-field-chips">
        {options.map((opt) => (
          <button
            key={opt.value}
            className={`cron-chip ${value === opt.value ? "active" : ""}`}
            onClick={() => { onChange(opt.value); setShowCustom(false); }}
            type="button"
          >
            {opt.label}
          </button>
        ))}
        {allowCustom !== false && (
          <button
            className={`cron-chip cron-chip--custom ${isCustom || showCustom ? "active" : ""}`}
            onClick={() => setShowCustom(!showCustom)}
            type="button"
          >
            Custom
          </button>
        )}
      </div>
      {(showCustom || isCustom) && allowCustom !== false && (
        <input
          className="routines-input cron-field-input"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={`e.g. */5, 1-5, 1,3,5`}
          spellCheck={false}
        />
      )}
    </div>
  );
}

function describeCronHuman(fields: CronFields): string {
  const parts: string[] = [];

  // Second
  if (fields.second !== "0") {
    if (fields.second === "*") parts.push("every second");
    else parts.push(`at second ${fields.second}`);
  }

  // Minute
  if (fields.minute === "*") parts.push("every minute");
  else if (fields.minute.startsWith("*/")) parts.push(`every ${fields.minute.slice(2)} minutes`);
  else parts.push(`at minute ${fields.minute}`);

  // Hour
  if (fields.hour === "*") { /* implied */ }
  else if (fields.hour.startsWith("*/")) parts.push(`every ${fields.hour.slice(2)} hours`);
  else {
    const h = parseInt(fields.hour, 10);
    if (!isNaN(h)) {
      const ampm = h >= 12 ? "PM" : "AM";
      const h12 = h === 0 ? 12 : h > 12 ? h - 12 : h;
      parts.push(`at ${h12} ${ampm}`);
    } else {
      parts.push(`hour ${fields.hour}`);
    }
  }

  // DOM
  if (fields.dom !== "*") parts.push(`on day ${fields.dom}`);

  // Month
  if (fields.month !== "*") {
    const monthNames: Record<string, string> = {
      "1": "Jan", "2": "Feb", "3": "Mar", "4": "Apr", "5": "May", "6": "Jun",
      "7": "Jul", "8": "Aug", "9": "Sep", "10": "Oct", "11": "Nov", "12": "Dec",
    };
    parts.push(`in ${monthNames[fields.month] || `month ${fields.month}`}`);
  }

  // DOW
  if (fields.dow !== "*") {
    const dowNames: Record<string, string> = {
      "0": "Sunday", "1": "Monday", "2": "Tuesday", "3": "Wednesday",
      "4": "Thursday", "5": "Friday", "6": "Saturday",
      "1-5": "weekdays", "0,6": "weekends", "1,3,5": "Mon/Wed/Fri", "2,4": "Tue/Thu",
    };
    parts.push(`on ${dowNames[fields.dow] || `DOW ${fields.dow}`}`);
  }

  return parts.length > 0 ? parts.join(", ") : "every second (no constraints)";
}

function describeCron(expr: string): string {
  if (!expr) return "";
  const preset = CRON_PRESETS.find((p) => p.cron === expr);
  if (preset) return preset.label;
  return describeCronHuman(parseCronFields(expr));
}

/** Compute the next N run times from a 6-field cron expression (client-side approximation). */
function computeNextRuns(cronExpr: string, timezone: string, count: number = 5): Date[] {
  if (!cronExpr) return [];
  const fields = parseCronFields(cronExpr);
  const results: Date[] = [];

  const matchField = (value: number, fieldExpr: string, max: number): boolean => {
    if (fieldExpr === "*") return true;
    if (fieldExpr === "L") return false; // skip "L" for simplicity
    // Handle step: */n or start/n
    if (fieldExpr.includes("/")) {
      const [base, stepStr] = fieldExpr.split("/");
      const step = parseInt(stepStr, 10);
      if (isNaN(step) || step <= 0) return false;
      const start = base === "*" ? 0 : parseInt(base, 10);
      if (isNaN(start)) return false;
      return (value - start) % step === 0 && value >= start;
    }
    // Handle range: a-b
    if (fieldExpr.includes("-") && !fieldExpr.includes(",")) {
      const [lo, hi] = fieldExpr.split("-").map(Number);
      return !isNaN(lo) && !isNaN(hi) && value >= lo && value <= hi;
    }
    // Handle list: a,b,c
    if (fieldExpr.includes(",")) {
      return fieldExpr.split(",").some((v) => matchField(value, v.trim(), max));
    }
    // Exact value
    const exact = parseInt(fieldExpr, 10);
    return !isNaN(exact) && value === exact;
  };

  // Start scanning from "now" and step forward by 1 minute (skip seconds for perf)
  const now = new Date();
  const cursor = new Date(now);
  cursor.setSeconds(0, 0);
  cursor.setMinutes(cursor.getMinutes() + 1);

  const maxIterations = 525960; // ~1 year of minutes
  for (let i = 0; i < maxIterations && results.length < count; i++) {
    const s = cursor.getSeconds();
    const min = cursor.getMinutes();
    const h = cursor.getHours();
    const dom = cursor.getDate();
    const mon = cursor.getMonth() + 1;
    const dow = cursor.getDay(); // 0=Sun

    if (
      matchField(s, fields.second, 59) &&
      matchField(min, fields.minute, 59) &&
      matchField(h, fields.hour, 23) &&
      matchField(dom, fields.dom, 31) &&
      matchField(mon, fields.month, 12) &&
      matchField(dow, fields.dow, 6)
    ) {
      results.push(new Date(cursor));
    }
    cursor.setMinutes(cursor.getMinutes() + 1);
  }
  return results;
}

function formatRelativeTime(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  const now = Date.now();
  const diff = d.getTime() - now;
  const abs = Math.abs(diff);
  if (abs < 60_000) return diff > 0 ? "in <1m" : "<1m ago";
  if (abs < 3600_000) {
    const m = Math.round(abs / 60_000);
    return diff > 0 ? `in ${m}m` : `${m}m ago`;
  }
  if (abs < 86_400_000) {
    const h = Math.round(abs / 3600_000);
    return diff > 0 ? `in ${h}h` : `${h}h ago`;
  }
  return d.toLocaleString();
}

/* ── Types ────────────────────────────────────────────── */

interface SessionInfo {
  id: string;
  title: string;
}

interface ProjectInfo {
  name: string;
  path: string;
  index: number;
  sessions: SessionInfo[];
}

interface Props {
  onClose: () => void;
  missions: Mission[];
  activeSessionId: string | null;
  autonomyMode: "observe" | "nudge" | "continue" | "autonomous";
  appState: { projects: ProjectInfo[]; active_project: number } | null;
}

/* ── Form state defaults ──────────────────────────────── */

interface FormState {
  name: string;
  trigger: RoutineTrigger;
  action: RoutineAction;
  enabled: boolean;
  cronExpr: string;
  timezone: string;
  targetMode: RoutineTargetMode;
  sessionId: string;
  projectIndex: number;
  prompt: string;
  providerId: string;
  modelId: string;
}

const DEFAULT_FORM: FormState = {
  name: "",
  trigger: "scheduled",
  action: "send_message",
  enabled: true,
  cronExpr: "0 0 */6 * * *",
  timezone: "UTC",
  targetMode: "existing_session",
  sessionId: "",
  projectIndex: 0,
  prompt: "",
  providerId: "",
  modelId: "",
};

function routineToForm(r: RoutineDefinition): FormState {
  return {
    name: r.name,
    trigger: r.trigger,
    action: r.action,
    enabled: r.enabled,
    cronExpr: r.cron_expr ?? "",
    timezone: r.timezone ?? "UTC",
    targetMode: r.target_mode ?? "existing_session",
    sessionId: r.session_id ?? "",
    projectIndex: r.project_index ?? 0,
    prompt: r.prompt ?? "",
    providerId: r.provider_id ?? "",
    modelId: r.model_id ?? "",
  };
}

/* ── Component ────────────────────────────────────────── */

export function RoutinesModal({ onClose, missions, activeSessionId, autonomyMode, appState }: Props) {
  const [routines, setRoutines] = useState<RoutineDefinition[]>([]);
  const [runs, setRuns] = useState<RoutineRunRecord[]>([]);
  const [loading, setLoading] = useState(true);

  // Create/edit form
  const [form, setForm] = useState<FormState>(DEFAULT_FORM);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [cronMode, setCronMode] = useState<"preset" | "builder" | "custom">("preset");
  const [saving, setSaving] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [showModelOverride, setShowModelOverride] = useState(false);

  // Provider/model data
  const providers = useProviders();

  const modalRef = useRef<HTMLDivElement>(null);

  useEscape(onClose);
  useFocusTrap(modalRef);

  // ── Data ──
  const load = useCallback(async () => {
    const resp = await fetchRoutines();
    setRoutines(resp.routines);
    setRuns(resp.runs);
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);

  // Listen for SSE routine updates
  useEffect(() => {
    const handler = () => { load(); };
    window.addEventListener("opman:routine-updated", handler);
    return () => window.removeEventListener("opman:routine-updated", handler);
  }, [load]);

  // ── Helpers ──
  const updateField = useCallback(<K extends keyof FormState>(key: K, val: FormState[K]) => {
    setForm((prev) => ({ ...prev, [key]: val }));
  }, []);

  const availableSessions = useMemo(() => {
    if (!appState?.projects) return [];
    return appState.projects.flatMap((proj) =>
      (proj.sessions ?? []).map((s) => ({
        id: s.id,
        label: `${proj.name}: ${s.title || s.id.slice(0, 8)}`,
        projectIndex: proj.index,
      }))
    );
  }, [appState]);

  const projects = useMemo(() => {
    return (appState?.projects ?? []).map((p) => ({ index: p.index, name: p.name, path: p.path }));
  }, [appState]);

  // Provider/model options for dropdowns
  const providerOptions = useMemo(() => {
    return providers.all.filter((p) => providers.connected.has(p.id));
  }, [providers.all, providers.connected]);

  const modelOptions = useMemo(() => {
    if (!form.providerId) return [];
    const provider = providers.all.find((p) => p.id === form.providerId);
    if (!provider) return [];
    return Object.values(provider.models);
  }, [providers.all, form.providerId]);

  // Compute next 5 runs preview for the cron expression in the form
  const nextRunsPreview = useMemo(() => {
    if (form.trigger !== "scheduled" || !form.cronExpr) return [];
    return computeNextRuns(form.cronExpr, form.timezone);
  }, [form.trigger, form.cronExpr, form.timezone]);

  // Helper to get a short schedule summary for a routine card
  const getScheduleSummary = useCallback((routine: RoutineDefinition): string => {
    if (routine.trigger === "manual") return "Manual only";
    if (routine.trigger === "on_session_idle") return "On idle";
    if (routine.trigger === "scheduled" && routine.cron_expr) return describeCron(routine.cron_expr);
    return routine.trigger;
  }, []);

  // Helper to get target summary for a routine card
  const getTargetSummary = useCallback((routine: RoutineDefinition): string => {
    if (routine.action !== "send_message") return "";
    if (routine.target_mode === "new_session") return "new session";
    if (routine.session_id) {
      // Try to find the session name
      const session = availableSessions.find((s) => s.id === routine.session_id);
      return session ? session.label : `session ${routine.session_id.slice(0, 8)}`;
    }
    return "current session";
  }, [availableSessions]);

  // ── Actions ──
  const resetForm = useCallback(() => {
    setForm(DEFAULT_FORM);
    setEditingId(null);
    setCronMode("preset");
    setShowModelOverride(false);
  }, []);

  const handleCreate = useCallback(async () => {
    if (!form.name.trim()) return;
    setSaving(true);
    setErrorMsg(null);
    try {
      const req: Record<string, unknown> = {
        name: form.name.trim(),
        trigger: form.trigger,
        action: form.action,
        enabled: form.enabled,
      };
      if (form.trigger === "scheduled" && form.cronExpr) {
        req.cron_expr = form.cronExpr;
        req.timezone = form.timezone || "UTC";
      }
      if (form.action === "send_message") {
        req.prompt = form.prompt || null;
        req.target_mode = form.targetMode;
        if (form.targetMode === "existing_session") {
          req.session_id = form.sessionId || activeSessionId || null;
        } else {
          req.project_index = form.projectIndex;
        }
      }
      if (form.providerId) req.provider_id = form.providerId;
      if (form.modelId) req.model_id = form.modelId;

      const routine = await createRoutine(req as any);
      setRoutines((prev) => [routine, ...prev]);
      resetForm();
      setShowCreate(false);
    } catch (e: any) {
      setErrorMsg(e?.message || "Failed to create routine");
    } finally {
      setSaving(false);
    }
  }, [form, activeSessionId, resetForm]);

  const handleSaveEdit = useCallback(async () => {
    if (!editingId || !form.name.trim()) return;
    setSaving(true);
    setErrorMsg(null);
    try {
      const req: Record<string, unknown> = {
        name: form.name.trim(),
        trigger: form.trigger,
        action: form.action,
        enabled: form.enabled,
      };
      if (form.trigger === "scheduled") {
        req.cron_expr = form.cronExpr || null;
        req.timezone = form.timezone || "UTC";
      } else {
        req.cron_expr = null;
      }
      if (form.action === "send_message") {
        req.prompt = form.prompt || null;
        req.target_mode = form.targetMode;
        if (form.targetMode === "existing_session") {
          req.session_id = form.sessionId || null;
        } else {
          req.project_index = form.projectIndex;
          req.session_id = null;
        }
      } else {
        req.prompt = null;
        req.target_mode = null;
      }
      if (form.providerId) req.provider_id = form.providerId;
      else req.provider_id = null;
      if (form.modelId) req.model_id = form.modelId;
      else req.model_id = null;

      const updated = await updateRoutine(editingId, req as any);
      setRoutines((prev) => prev.map((r) => (r.id === updated.id ? updated : r)));
      resetForm();
    } catch (e: any) {
      setErrorMsg(e?.message || "Failed to save routine");
    } finally {
      setSaving(false);
    }
  }, [editingId, form, resetForm]);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await deleteRoutine(id);
      setRoutines((prev) => prev.filter((r) => r.id !== id));
      if (editingId === id) resetForm();
      setConfirmDeleteId(null);
    } catch (e: any) {
      setErrorMsg(e?.message || "Failed to delete routine");
      setConfirmDeleteId(null);
    }
  }, [editingId, resetForm]);

  const handleRun = useCallback(async (id: string) => {
    setRunningId(id);
    setErrorMsg(null);
    try {
      const run = await runRoutine(id);
      setRuns((prev) => [run, ...prev]);
    } catch (e: any) {
      setErrorMsg(e?.message || "Failed to run routine");
    } finally {
      setRunningId(null);
    }
  }, []);

  const handleToggleEnabled = useCallback(async (routine: RoutineDefinition) => {
    try {
      const updated = await updateRoutine(routine.id, { enabled: !routine.enabled });
      setRoutines((prev) => prev.map((r) => (r.id === updated.id ? updated : r)));
    } catch (e: any) {
      setErrorMsg(e?.message || "Failed to toggle routine");
    }
  }, []);

  const startEdit = useCallback((routine: RoutineDefinition) => {
    setForm(routineToForm(routine));
    setEditingId(routine.id);
    setShowCreate(false);
    setShowModelOverride(!!(routine.provider_id || routine.model_id));
    const presetMatch = CRON_PRESETS.find((p) => p.cron === (routine.cron_expr ?? ""));
    setCronMode(presetMatch ? "preset" : "builder");
  }, []);

  const cancelEdit = useCallback(() => {
    resetForm();
  }, [resetForm]);

  const runsForRoutine = useCallback(
    (id: string) => runs.filter((r) => r.routine_id === id).slice(0, 5),
    [runs],
  );

  // ── Is the form for scheduled trigger? ──
  const isScheduled = form.trigger === "scheduled";

  // ── Render ──
  return (
    <div className="routines-overlay" onClick={onClose}>
      <div
        ref={modalRef}
        className="routines-modal"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="routines-header">
          <div className="routines-header-left">
            <Clock3 size={16} />
            <h3>Routines</h3>
            <span className="routines-count">{routines.length}</span>
          </div>
          <div className="routines-header-actions">
            <button
              className="routines-add-btn"
              onClick={() => { resetForm(); setShowCreate(!showCreate); }}
              title="Create routine"
            >
              <Plus size={14} /> New
            </button>
            <button onClick={onClose} aria-label="Close routines">
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Scrollable content area (form + list) */}
        <div className="routines-scrollable">

        {/* Create / Edit Form */}
        {(showCreate || editingId) && (
          <div className="routines-form">
            <div className="routines-form-title">
              {editingId ? "Edit Routine" : "New Routine"}
            </div>

            {/* ── Section 1: Basics ── */}
            <div className="routines-form-section">
              <div className="routines-form-section-header">Basics</div>

              {/* Name */}
              <input
                className="routines-input"
                value={form.name}
                onChange={(e) => updateField("name", e.target.value)}
                placeholder="Routine name"
                autoFocus
              />

              {/* Trigger */}
              <div className="routines-form-group">
                <label className="routines-label">Trigger</label>
                <select
                  className="routines-select"
                  value={form.trigger === "on_session_idle" ? form.trigger : form.trigger}
                  onChange={(e) => {
                    const t = e.target.value as RoutineTrigger;
                    updateField("trigger", t);
                  }}
                >
                  <option value="scheduled">Scheduled (Cron)</option>
                  <option value="manual">Manual</option>
                  {/* Show on_session_idle only if it was already set (backward compat) */}
                  {form.trigger === "on_session_idle" && (
                    <option value="on_session_idle">On Session Idle (legacy)</option>
                  )}
                </select>
                {form.trigger === "on_session_idle" && (
                  <span className="routines-hint">Fires when the bound session becomes idle</span>
                )}
              </div>
            </div>

            {/* ── Section 2: Schedule (only if trigger=scheduled) ── */}
            {isScheduled && (
              <div className="routines-form-section">
                <div className="routines-form-section-header">Schedule</div>
                <div className="routines-cron-section">
                  <div className="routines-form-row">
                    <div className="routines-form-group routines-form-group--wide">
                      <div className="routines-cron-tabs">
                        <button
                          className={`routines-cron-tab ${cronMode === "preset" ? "active" : ""}`}
                          onClick={() => setCronMode("preset")}
                        >
                          Presets
                        </button>
                        <button
                          className={`routines-cron-tab ${cronMode === "builder" ? "active" : ""}`}
                          onClick={() => setCronMode("builder")}
                        >
                          Builder
                        </button>
                        <button
                          className={`routines-cron-tab ${cronMode === "custom" ? "active" : ""}`}
                          onClick={() => setCronMode("custom")}
                        >
                          Raw
                        </button>
                      </div>
                    </div>
                    <div className="routines-form-group">
                      <label className="routines-label">Timezone</label>
                      <select
                        className="routines-select"
                        value={form.timezone}
                        onChange={(e) => updateField("timezone", e.target.value)}
                      >
                        {TIMEZONE_OPTIONS.map((tz) => (
                          <option key={tz} value={tz}>{tz}</option>
                        ))}
                      </select>
                    </div>
                  </div>

                  {cronMode === "preset" && (
                    <div className="routines-cron-presets">
                      {CRON_PRESETS.map((p) => (
                        <button
                          key={p.cron}
                          className={`routines-cron-preset ${form.cronExpr === p.cron ? "active" : ""}`}
                          onClick={() => updateField("cronExpr", p.cron)}
                        >
                          {p.label}
                        </button>
                      ))}
                    </div>
                  )}

                  {cronMode === "builder" && (() => {
                    const fields = parseCronFields(form.cronExpr);
                    const setField = (field: keyof CronFields, val: string) => {
                      const updated = { ...fields, [field]: val };
                      updateField("cronExpr", assembleCron(updated));
                    };
                    return (
                      <div className="cron-builder">
                        <CronFieldPicker label="Second" value={fields.second} options={[
                          { label: "0 (default)", value: "0" },
                          { label: "Every sec", value: "*" },
                          { label: "*/10", value: "*/10" },
                          { label: "*/15", value: "*/15" },
                          { label: "*/30", value: "*/30" },
                        ]} onChange={(v) => setField("second", v)} />
                        <CronFieldPicker label="Minute" value={fields.minute} options={MINUTE_OPTIONS} onChange={(v) => setField("minute", v)} />
                        <CronFieldPicker label="Hour" value={fields.hour} options={HOUR_OPTIONS} onChange={(v) => setField("hour", v)} />
                        <CronFieldPicker label="Day of Month" value={fields.dom} options={DOM_OPTIONS} onChange={(v) => setField("dom", v)} />
                        <CronFieldPicker label="Month" value={fields.month} options={MONTH_OPTIONS} onChange={(v) => setField("month", v)} />
                        <CronFieldPicker label="Day of Week" value={fields.dow} options={DOW_OPTIONS} onChange={(v) => setField("dow", v)} />
                        <div className="cron-builder-summary">
                          <span className="cron-builder-summary-label">Result:</span>
                          <code className="cron-builder-summary-expr">{form.cronExpr}</code>
                          <span className="cron-builder-summary-desc">{describeCronHuman(fields)}</span>
                        </div>
                      </div>
                    );
                  })()}

                  {cronMode === "custom" && (
                    <div className="routines-cron-custom">
                      <input
                        className="routines-input routines-cron-input"
                        value={form.cronExpr}
                        onChange={(e) => updateField("cronExpr", e.target.value)}
                        placeholder="sec min hour dom month dow (e.g. 0 0 9 * * 1-5)"
                        spellCheck={false}
                      />
                      <span className="routines-cron-hint">
                        6-field cron: sec min hour day-of-month month day-of-week
                      </span>
                      {form.cronExpr && (
                        <span className="routines-cron-hint">
                          Reads as: {describeCron(form.cronExpr)}
                        </span>
                      )}
                    </div>
                  )}

                  {/* Next 5 runs preview */}
                  {form.cronExpr && nextRunsPreview.length > 0 && (
                    <div className="routines-next-runs">
                      <div className="routines-next-runs-title">Next {nextRunsPreview.length} runs</div>
                      <ul className="routines-next-runs-list">
                        {nextRunsPreview.map((d, i) => (
                          <li key={i} className="routines-next-runs-item">
                            {d.toLocaleString(undefined, {
                              weekday: "short",
                              month: "short",
                              day: "numeric",
                              hour: "2-digit",
                              minute: "2-digit",
                            })}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* ── Section 3: Message ── */}
            <div className="routines-form-section">
              <div className="routines-form-section-header">Message</div>
              <textarea
                className="routines-textarea"
                value={form.prompt}
                onChange={(e) => updateField("prompt", e.target.value)}
                placeholder="Enter the message to send to the session..."
                rows={3}
              />
            </div>

            {/* ── Section 4: Target ── */}
            <div className="routines-form-section">
              <div className="routines-form-section-header">Target</div>
              <div className="routines-form-row">
                <div className="routines-form-group">
                  <label className="routines-label">Target Mode</label>
                  <select
                    className="routines-select"
                    value={form.targetMode}
                    onChange={(e) => updateField("targetMode", e.target.value as RoutineTargetMode)}
                    disabled={form.trigger === "on_session_idle"}
                  >
                    <option value="existing_session">Existing Session</option>
                    {form.trigger !== "on_session_idle" && (
                      <option value="new_session">New Session</option>
                    )}
                  </select>
                </div>
                {form.targetMode === "existing_session" ? (
                  <div className="routines-form-group">
                    <label className="routines-label">Session</label>
                    <select
                      className="routines-select"
                      value={form.sessionId}
                      onChange={(e) => updateField("sessionId", e.target.value)}
                    >
                      <option value="">
                        {activeSessionId ? `Current (${activeSessionId.slice(0, 8)})` : "Select session"}
                      </option>
                      {availableSessions.map((s) => (
                        <option key={s.id} value={s.id}>{s.label}</option>
                      ))}
                    </select>
                  </div>
                ) : (
                  <div className="routines-form-group">
                    <label className="routines-label">Project</label>
                    <select
                      className="routines-select"
                      value={form.projectIndex}
                      onChange={(e) => updateField("projectIndex", Number(e.target.value))}
                    >
                      {projects.map((p) => (
                        <option key={p.index} value={p.index}>{p.name}</option>
                      ))}
                    </select>
                  </div>
                )}
              </div>
            </div>

            {/* ── Section 5: Model Override (collapsed by default) ── */}
            <div className="routines-form-section">
              <label className="routines-form-section-toggle">
                <input
                  type="checkbox"
                  checked={showModelOverride}
                  onChange={(e) => {
                    setShowModelOverride(e.target.checked);
                    if (!e.target.checked) {
                      updateField("providerId", "");
                      updateField("modelId", "");
                    }
                  }}
                />
                <span className="routines-form-section-header routines-form-section-header--toggle">
                  Customize model
                </span>
              </label>
              {showModelOverride && (
                <div className="routines-form-row">
                  <div className="routines-form-group">
                    <label className="routines-label">Provider</label>
                    <select
                      className="routines-select"
                      value={form.providerId}
                      onChange={(e) => {
                        updateField("providerId", e.target.value);
                        updateField("modelId", "");
                      }}
                    >
                      <option value="">Default provider</option>
                      {providerOptions.map((p) => (
                        <option key={p.id} value={p.id}>{p.name || p.id}</option>
                      ))}
                    </select>
                  </div>
                  <div className="routines-form-group">
                    <label className="routines-label">Model</label>
                    <select
                      className="routines-select"
                      value={form.modelId}
                      onChange={(e) => updateField("modelId", e.target.value)}
                      disabled={!form.providerId}
                    >
                      <option value="">Default model</option>
                      {modelOptions.map((m) => (
                        <option key={m.id} value={m.id}>{m.name || m.id}</option>
                      ))}
                    </select>
                    {!form.providerId && (
                      <span className="routines-cron-hint">Select a provider first</span>
                    )}
                  </div>
                </div>
              )}
            </div>

            {/* Error banner */}
            {errorMsg && (
              <div className="routines-error-banner">
                <AlertTriangle size={13} />
                <span>{errorMsg}</span>
                <button onClick={() => setErrorMsg(null)} className="routines-error-dismiss">&times;</button>
              </div>
            )}

            {/* Enabled toggle + submit */}
            <div className="routines-form-footer">
              <label className="routines-toggle">
                <input
                  type="checkbox"
                  checked={form.enabled}
                  onChange={(e) => updateField("enabled", e.target.checked)}
                />
                <span className="routines-toggle-label">
                  {form.enabled ? "Enabled" : "Disabled"}
                </span>
              </label>
              <div className="routines-form-actions">
                <button
                  className="routines-btn routines-btn--muted"
                  onClick={() => { editingId ? cancelEdit() : setShowCreate(false); setErrorMsg(null); }}
                >
                  Cancel
                </button>
                <button
                  className="routines-btn routines-btn--primary"
                  onClick={editingId ? handleSaveEdit : handleCreate}
                  disabled={!form.name.trim() || !form.prompt.trim() || saving}
                >
                  {saving && <span className="routines-spinner" />}
                  <Save size={14} />
                  {editingId ? "Save Changes" : "Create Routine"}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Routine list */}
        <div className="routines-body">
          {loading ? (
            <div className="routines-empty">Loading routines...</div>
          ) : routines.length === 0 && !showCreate ? (
            <div className="routines-empty-state">
              <Calendar size={32} />
              <div className="routines-empty-title">No routines yet</div>
              <div className="routines-empty-desc">
                Create a routine to automatically send messages to sessions on a schedule.
              </div>
              <button
                className="routines-btn routines-btn--primary"
                onClick={() => { resetForm(); setShowCreate(true); }}
              >
                <Plus size={14} /> Create your first routine
              </button>
            </div>
          ) : (
            routines.map((routine) => {
              const isExpanded = expandedId === routine.id;
              const isEditing = editingId === routine.id;
              const rRuns = runsForRoutine(routine.id);
              const isRunning = runningId === routine.id;

              return (
                <div
                  key={routine.id}
                  className={`routines-card ${!routine.enabled ? "routines-card--disabled" : ""} ${isEditing ? "routines-card--editing" : ""}`}
                >
                  <div className="routines-card-header">
                    <button
                      className="routines-card-expand"
                      onClick={() => setExpandedId(isExpanded ? null : routine.id)}
                    >
                      {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </button>

                    <div className="routines-card-info" onClick={() => setExpandedId(isExpanded ? null : routine.id)}>
                      <div className="routines-card-name">
                        {routine.action === "send_message" && <Send size={13} className="routines-card-icon" />}
                        {routine.action !== "send_message" && <Clock3 size={13} className="routines-card-icon" />}
                        {routine.name}
                      </div>
                      {routine.prompt && (
                        <div className="routines-card-prompt-preview">
                          {routine.prompt.length > 60 ? routine.prompt.slice(0, 60) + "…" : routine.prompt}
                        </div>
                      )}
                      <div className="routines-card-meta">
                        <span className={`routines-trigger-badge routines-trigger-badge--${routine.trigger}`}>
                          {routine.trigger === "scheduled" ? "scheduled" : routine.trigger.replace(/_/g, " ")}
                        </span>
                        <span className="routines-schedule-summary">{getScheduleSummary(routine)}</span>
                        {routine.action === "send_message" && (
                          <span className="routines-target-summary">{getTargetSummary(routine)}</span>
                        )}
                        {routine.last_error && (
                          <span className="routines-error-badge" title={routine.last_error}>
                            <AlertTriangle size={11} /> error
                          </span>
                        )}
                      </div>
                    </div>

                    <div className="routines-card-actions">
                      <button
                        className={`routines-icon-btn ${routine.enabled ? "routines-icon-btn--enabled" : "routines-icon-btn--disabled-state"}`}
                        onClick={() => handleToggleEnabled(routine)}
                        title={routine.enabled ? "Disable" : "Enable"}
                      >
                        {routine.enabled ? <Power size={14} /> : <PowerOff size={14} />}
                      </button>
                      <button
                        className="routines-icon-btn"
                        onClick={() => startEdit(routine)}
                        title="Edit"
                      >
                        <Pencil size={14} />
                      </button>
                      <button
                        className="routines-icon-btn routines-icon-btn--run"
                        onClick={() => handleRun(routine.id)}
                        disabled={isRunning}
                        title="Run now"
                      >
                        <Play size={14} />
                      </button>
                      <button
                        className="routines-icon-btn routines-icon-btn--danger"
                        onClick={() => setConfirmDeleteId(routine.id)}
                        title="Delete"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </div>

                  {isExpanded && (
                    <div className="routines-card-detail">
                      {/* Schedule info */}
                      {routine.trigger === "scheduled" && (
                        <div className="routines-detail-row">
                          <span className="routines-detail-label">Schedule</span>
                          <span className="routines-detail-value">
                            {routine.cron_expr || "—"} ({routine.timezone || "UTC"})
                          </span>
                        </div>
                      )}

                      {/* Target */}
                      {routine.action === "send_message" && (
                        <>
                          <div className="routines-detail-row">
                            <span className="routines-detail-label">Target</span>
                            <span className="routines-detail-value">
                              {routine.target_mode === "new_session"
                                ? `New session (project #${routine.project_index ?? 0})`
                                : routine.session_id
                                  ? `Session ${routine.session_id.slice(0, 12)}...`
                                  : "Current session"}
                            </span>
                          </div>
                          {routine.prompt && (
                            <div className="routines-detail-row routines-detail-row--block">
                              <span className="routines-detail-label">Prompt</span>
                              <div className="routines-detail-prompt">{routine.prompt}</div>
                            </div>
                          )}
                        </>
                      )}

                      {/* Provider / Model */}
                      {(routine.provider_id || routine.model_id) && (
                        <div className="routines-detail-row">
                          <span className="routines-detail-label">Model</span>
                          <span className="routines-detail-value">
                            {routine.provider_id && `${routine.provider_id}/`}{routine.model_id || "default"}
                          </span>
                        </div>
                      )}

                      {/* Timing */}
                      <div className="routines-detail-row">
                        <span className="routines-detail-label">Last run</span>
                        <span className="routines-detail-value">{formatRelativeTime(routine.last_run_at)}</span>
                      </div>
                      <div className="routines-detail-row">
                        <span className="routines-detail-label">Next run</span>
                        <span className="routines-detail-value">{formatRelativeTime(routine.next_run_at)}</span>
                      </div>

                      {routine.last_error && (
                        <div className="routines-detail-row routines-detail-row--error">
                          <span className="routines-detail-label">Last error</span>
                          <span className="routines-detail-value routines-detail-value--error">
                            {routine.last_error}
                          </span>
                        </div>
                      )}

                      {/* Recent runs */}
                      {rRuns.length > 0 && (
                        <div className="routines-runs-section">
                          <div className="routines-runs-title">Recent Runs</div>
                          {rRuns.map((run) => (
                            <div key={run.id} className="routines-run-row">
                              <span className={`routines-run-status routines-run-status--${run.status === "completed" ? "success" : run.status === "failed" ? "error" : run.status}`}>
                                {run.status === "completed" && <Check size={11} />}
                                {run.status === "failed" && <AlertTriangle size={11} />}
                                {run.status}
                              </span>
                              <span className="routines-run-summary" title={run.summary}>
                                {run.summary.length > 60 ? run.summary.slice(0, 60) + "..." : run.summary}
                              </span>
                              {run.duration_ms != null && (
                                <span className="routines-run-duration">{run.duration_ms}ms</span>
                              )}
                              <span className="routines-run-time">
                                {new Date(run.created_at).toLocaleTimeString()}
                              </span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
        </div>
        {/* Error banner (outside form context) */}
        {errorMsg && !(showCreate || editingId) && (
          <div className="routines-error-banner routines-error-banner--global">
            <AlertTriangle size={13} />
            <span>{errorMsg}</span>
            <button onClick={() => setErrorMsg(null)} className="routines-error-dismiss">&times;</button>
          </div>
        )}

        {/* Delete confirmation dialog */}
        {confirmDeleteId && (
          <div className="routines-confirm-overlay" onClick={() => setConfirmDeleteId(null)}>
            <div className="routines-confirm-dialog" onClick={(e) => e.stopPropagation()}>
              <div className="routines-confirm-title">Delete Routine</div>
              <div className="routines-confirm-msg">
                Are you sure you want to delete "{routines.find((r) => r.id === confirmDeleteId)?.name}"? This action cannot be undone.
              </div>
              <div className="routines-confirm-actions">
                <button className="routines-btn routines-btn--muted" onClick={() => setConfirmDeleteId(null)}>Cancel</button>
                <button className="routines-btn routines-btn--danger" onClick={() => handleDelete(confirmDeleteId)}>
                  <Trash2 size={13} /> Delete
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
