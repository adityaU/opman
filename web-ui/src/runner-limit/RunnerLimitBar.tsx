import React, { useMemo, useState } from "react";
import { AlertTriangle, ArrowRightLeft, Gauge, Settings2, X } from "lucide-react";
import type { Message } from "../types";
import { runnerLabel } from "../engine-picker/paletteRows";
import { shortModelName } from "../prompt-input/helpers";
import { formatResetIn } from "../usage/format";
import { openSettings } from "../settings-page/useSettingsRoute";
import { optionKey, type LimitTarget } from "./chooseTarget";
import type { FallbackModel } from "./fallbacks";
import type { RunnerLimit } from "./limitState";
import { useLimitSwitch } from "./useLimitSwitch";
import { useRunnerLimit } from "./useRunnerLimit";
import { useProviders } from "../hooks/useProviders";

export interface RunnerLimitBarProps {
  readonly sessionId: string | null;
  readonly runner: string;
  readonly availableRunners: readonly string[];
  readonly messages?: readonly Message[];
  readonly selectedModel: FallbackModel | null;
  readonly selectedPermission?: string;
  readonly isBusy: boolean;
  readonly onRunnerChange?: (runner: string) => void;
  readonly onModelSelected?: (modelId: string, providerId: string) => void;
  readonly onPermissionChange?: (permission: string) => void;
  readonly onSend: (text: string) => Promise<boolean>;
}

function targetLabel(target: Pick<LimitTarget, "runner" | "model">): string {
  const model = target.model ? shortModelName(target.model.modelID) : "default model";
  return `${runnerLabel(target.runner)} · ${model}`;
}

function optionLabel(option: LimitTarget): string {
  return option.leftPercent === null
    ? targetLabel(option)
    : `${targetLabel(option)} — ${option.leftPercent}% left`;
}

function headline(runner: string, limit: RunnerLimit): string {
  const name = runnerLabel(runner);
  if (limit.failed?.kind === "rate") return `${name} is rate-limited`;
  if (limit.failed) return `${name} hit its usage limit`;
  if (limit.level === "out") return `${name} has run out`;
  return `${name} has ${limit.leftPercent ?? 0}% left`;
}

function detail(limit: RunnerLimit, now: number): string | null {
  const parts = [limit.window?.label, formatResetIn(limit.resetsAt, now)].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : null;
}

/**
 * The composer's note about its runner running out, and the way off it.
 *
 * "Ask" shows the note with the best target preselected in a picker, and switching
 * re-sends a prompt that failed on the limit. "Auto" is done by the server — it moves the
 * next message, or continues a failed turn, on the first pair of the user's list, with
 * or without a browser open — so here it only says where the conversation is going.
 * With nothing listed, "auto" falls back to offering, like "ask": a runner the user never
 * chose is never taken on its own. "Off" shows nothing.
 */
export function RunnerLimitBar(props: RunnerLimitBarProps) {
  const { sessionId, runner, availableRunners, messages, isBusy } = props;
  const view = useRunnerLimit(sessionId, runner, availableRunners, messages, props.selectedModel);
  const { limit, options, policy, dismissed, dismiss } = view;
  const { switchTo, switching, error, clearError } = useLimitSwitch({
    currentRunner: runner,
    hasSession: sessionId !== null,
    selectedModel: props.selectedModel,
    onRunnerChange: props.onRunnerChange,
    onModelSelected: props.onModelSelected,
    selectedPermission: props.selectedPermission,
    onPermissionChange: props.onPermissionChange,
    onSend: props.onSend,
  });
  const [picked, setPicked] = useState<string | null>(null);
  const [pickedPermission, setPickedPermission] = useState<{ key: string; permission: string | null } | null>(null);

  const chosen = useMemo(
    () => options.find((option) => optionKey(option) === picked) ?? options[0] ?? null,
    [options, picked],
  );
  const { permissionModes, loading: permissionsLoading } = useProviders(chosen?.runner ?? runner);
  const permission = chosen && pickedPermission?.key === optionKey(chosen)
    ? pickedPermission.permission : chosen?.permission ?? null;
  const permissionSupported = !permission || Boolean(permissionModes?.some((mode) => mode.value === permission));
  // Only a turn that failed and is not running can be re-sent; the prompt is the one it lost.
  const retryPrompt = !isBusy && limit.failed?.prompt ? limit.failed.prompt : null;
  const go = (target: LimitTarget) => {
    const compatibleSource = permissionModes?.some(mode => mode.value === props.selectedPermission)
      ? props.selectedPermission : null;
    switchTo({ ...target, permission: permission ?? compatibleSource }, retryPrompt);
  };

  if (policy === "off" || dismissed) return null;
  if (error) {
    return (
      <div className="rlim-bar rlim-bar--out" role="alert">
        <AlertTriangle size={13} className="rlim-icon" aria-hidden="true" />
        <span className="rlim-text">{error}</span>
        <button type="button" className="rlim-icon-btn" aria-label="Hide" onClick={clearError}>
          <X size={13} />
        </button>
      </div>
    );
  }
  if (limit.level === "ok") return null;

  const auto = policy === "auto" && options[0]?.source === "preselected" ? options[0] : null;
  if (auto) {
    return (
      <div className="rlim-bar rlim-bar--moved" role="status">
        <ArrowRightLeft size={13} className="rlim-icon" aria-hidden="true" />
        <span className="rlim-text">
          <span className="rlim-head">{headline(runner, limit)}</span>
          <span className="rlim-sub">
            {limit.failed ? "Continuing on" : "Your next message goes to"} {targetLabel(auto)}
          </span>
        </span>
        <span className="rlim-actions">
          <button type="button" className="rlim-icon-btn" aria-label="Usage limit settings"
            title="Choose where chats go when a runner runs low" onClick={() => openSettings("limits")}>
            <Settings2 size={13} />
          </button>
          <button type="button" className="rlim-icon-btn" aria-label="Hide" onClick={() => dismiss()}>
            <X size={13} />
          </button>
        </span>
      </div>
    );
  }


  const out = limit.level === "out";
  const sub = detail(limit, Date.now());
  return (
    <div className={`rlim-bar rlim-bar--${limit.level}`} role="status">
      {out
        ? <AlertTriangle size={13} className="rlim-icon" aria-hidden="true" />
        : <Gauge size={13} className="rlim-icon" aria-hidden="true" />}
      <span className="rlim-text">
        <span className="rlim-head">{headline(runner, limit)}</span>
        {sub && <span className="rlim-sub">{sub}</span>}
        {options.length === 0 && <span className="rlim-sub">No other runner has room right now</span>}
      </span>
      <span className="rlim-actions">
        {options.length > 0 && chosen && (
          <>
            <select
              className="rlim-select"
              aria-label="Move this chat to"
              value={optionKey(chosen)}
              disabled={switching}
              onChange={(event) => { setPicked(event.target.value); setPickedPermission(null); }}
            >
              {options.map((option) => (
                <option key={optionKey(option)} value={optionKey(option)}>
                  {option.source === "preselected" ? "★ " : ""}{optionLabel(option)}
                </option>
              ))}
            </select>
            {(Boolean(permissionModes?.length) || Boolean(permission)) && (
              <select className="rlim-select" aria-label="Handoff permission"
                value={permission ?? ""} disabled={switching || permissionsLoading}
                onChange={(event) => setPickedPermission({ key: optionKey(chosen), permission: event.target.value || null })}>
                <option value="">Keep compatible permission</option>
                {permission && !permissionSupported && <option value={permission} disabled>Unavailable: {permission}</option>}
                {permissionModes?.map((mode) => <option key={mode.value} value={mode.value}>{mode.label}</option>)}
              </select>
            )}
            <button type="button" className="rlim-btn rlim-btn--primary"
              disabled={switching || !permissionSupported || permissionsLoading} onClick={() => go(chosen)}>
              {retryPrompt ? "Switch & retry" : "Switch"}
            </button>
          </>
        )}
        <button type="button" className="rlim-btn" onClick={() => dismiss()}>Not now</button>
        <button type="button" className="rlim-icon-btn" aria-label="Usage limit settings"
          title="Choose where chats go when a runner runs low" onClick={() => openSettings("limits")}>
          <Settings2 size={13} />
        </button>
      </span>
    </div>
  );
}
