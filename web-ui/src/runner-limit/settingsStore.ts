/**
 * The "runner running low" settings, as one process-wide copy of what the server holds.
 *
 * The server owns them (`/api/runner-limits`): the automatic switch runs there, so it
 * works for a session left going with no tab open. This store loads them once on first
 * use, writes edits back optimistically, and re-reads when another tab or device changes
 * them (`runner_limits_changed` over the app event stream).
 *
 * Settings saved by an earlier build lived in this browser's localStorage. They are
 * copied up once, when the server still has nothing of its own.
 */
import {
  fetchRunnerLimits,
  RUNNER_LIMITS_CHANGED,
  saveRunnerLimits,
  type LimitPolicy,
  type RunnerLimitSettings,
} from "../api/runnerLimits";
import { NEAR_PERCENT } from "./limitState";
import { parseFallbacks } from "./legacyFallbacks";

export const DEFAULT_SETTINGS: RunnerLimitSettings = { policy: "ask", nearPercent: NEAR_PERCENT, pairs: [] };

/** Where an earlier build kept these, per browser. Read once for the move to the server. */
export const LEGACY_POLICY_KEY = "opman-runner-limit-policy";
export const LEGACY_FALLBACKS_KEY = "opman-runner-fallbacks";

let snapshot: RunnerLimitSettings = DEFAULT_SETTINGS;
let started = false;
const listeners = new Set<() => void>();

function publish(next: RunnerLimitSettings): void {
  snapshot = next;
  for (const listener of listeners) listener();
}

const isPolicy = (value: unknown): value is LimitPolicy =>
  value === "ask" || value === "auto" || value === "off";

function isDefault(settings: RunnerLimitSettings): boolean {
  return settings.policy === DEFAULT_SETTINGS.policy
    && settings.nearPercent === DEFAULT_SETTINGS.nearPercent
    && settings.pairs.length === 0;
}

/** This browser's old copy, if it has one worth keeping. Clears it either way. */
function takeLegacy(): RunnerLimitSettings | null {
  try {
    const policy = localStorage.getItem(LEGACY_POLICY_KEY);
    const fallbacks = localStorage.getItem(LEGACY_FALLBACKS_KEY);
    localStorage.removeItem(LEGACY_POLICY_KEY);
    localStorage.removeItem(LEGACY_FALLBACKS_KEY);
    if (!policy && !fallbacks) return null;
    const legacy = { ...DEFAULT_SETTINGS, ...parseFallbacks(fallbacks), policy: isPolicy(policy) ? policy : DEFAULT_SETTINGS.policy };
    return isDefault(legacy) ? null : legacy;
  } catch {
    return null;
  }
}

export async function reloadSettings(): Promise<void> {
  try {
    const remote = await fetchRunnerLimits();
    const legacy = isDefault(remote) ? takeLegacy() : null;
    publish(legacy ? await saveRunnerLimits(legacy) : remote);
  } catch {
    /* offline or older server: keep what is on screen */
  }
}

function start(): void {
  if (started) return;
  started = true;
  window.addEventListener(RUNNER_LIMITS_CHANGED, () => void reloadSettings());
  void reloadSettings();
}

export function subscribeSettings(listener: () => void): () => void {
  start();
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getSettings(): RunnerLimitSettings {
  return snapshot;
}

/** Apply an edit now and save it; a failed save puts the server's copy back. */
export function updateSettings(change: Partial<RunnerLimitSettings>): void {
  const next = { ...snapshot, ...change };
  publish(next);
  saveRunnerLimits(next).then(publish, () => void reloadSettings());
}

/** Tests only. */
export function resetSettingsStore(next: RunnerLimitSettings = DEFAULT_SETTINGS): void {
  snapshot = next;
  started = true;
  listeners.clear();
}
