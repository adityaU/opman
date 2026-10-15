/**
 * `GET`/`PUT /api/runner-limits` — what happens when a runner runs low.
 *
 * Stored by the server so the automatic switch also works with no browser open, and so
 * every device sees the same list.
 */
import { apiFetch, apiPut } from "./client";

export type LimitPolicy = "ask" | "auto" | "off";

export interface FallbackModel {
  readonly providerID: string;
  readonly modelID: string;
}

export interface FallbackPair {
  readonly runner: string;
  /** Null: whatever the runner picks by default. */
  readonly model: FallbackModel | null;
}

export interface RunnerLimitSettings {
  readonly policy: LimitPolicy;
  /** Percent left at or under which a runner counts as running low. 1–99. */
  readonly nearPercent: number;
  readonly pairs: readonly FallbackPair[];
}

/** Window event fired when another tab or device changed the settings. */
export const RUNNER_LIMITS_CHANGED = "opman:runner-limits-changed";

export function fetchRunnerLimits(): Promise<RunnerLimitSettings> {
  return apiFetch<RunnerLimitSettings>("/runner-limits");
}

export function saveRunnerLimits(settings: RunnerLimitSettings): Promise<RunnerLimitSettings> {
  return apiPut<RunnerLimitSettings>("/runner-limits", settings);
}
