/**
 * `/api/runners/settings` — switch the built-in runners (OpenCode, Claude Code) on and
 * off, and choose where new sessions go. ACP agents are switched in `acp.json` instead.
 */
import { ApiError, apiFetch, apiPut } from "./client";

export interface BuiltinRunnerView {
  readonly id: string;
  readonly name: string;
  /** Its binary answers `--version`; one that does not cannot be switched on. */
  readonly installed: boolean;
  readonly enabled: boolean;
  /** opman started on it; it serves the TUI and the process-wide base URL. */
  readonly primary: boolean;
}

export interface RunnersView {
  readonly builtin: readonly BuiltinRunnerView[];
  /** Where a new session goes when nobody names a runner. */
  readonly default: string;
  /** Runners that can be the default: served and switched on. */
  readonly choices: readonly string[];
}

export function fetchRunnerSettings(): Promise<RunnersView> {
  return apiFetch<RunnersView>("/runners/settings");
}

export function setRunnerEnabled(id: string, enabled: boolean): Promise<RunnersView> {
  return apiPut<RunnersView>(`/runners/settings/${encodeURIComponent(id)}`, { enabled });
}

export function setDefaultRunner(runner: string): Promise<RunnersView> {
  return apiPut<RunnersView>("/runners/default", { runner });
}

/**
 * The words to show for a failed send: the server's own reason when the runner is
 * switched off (it says how to turn it back on), else `fallback`.
 */
export function sendFailureMessage(error: unknown, fallback: string): string {
  return error instanceof ApiError && error.code === "runner_disabled" ? error.message : fallback;
}
