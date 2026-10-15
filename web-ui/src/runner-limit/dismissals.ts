/**
 * "Not now", remembered per session until the limit window resets.
 *
 * Dismissing the near-limit warning does not silence the out-of-limit one: running out is
 * new information. Dismissing "out" covers "near" too.
 */
import type { LimitLevel } from "./limitState";

export const DISMISS_KEY = "opman-runner-limit-dismissed";
/** Used when the provider gave no reset time. */
export const DEFAULT_DISMISS_MS = 60 * 60_000;
const MAX_ENTRIES = 200;

export interface Dismissal {
  readonly level: Exclude<LimitLevel, "ok">;
  readonly until: number;
}

type Table = Record<string, Dismissal>;

function read(): Table {
  try {
    const raw = localStorage.getItem(DISMISS_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    return typeof parsed === "object" && parsed !== null ? (parsed as Table) : {};
  } catch {
    return {};
  }
}

function write(table: Table, now: number): void {
  const live = Object.entries(table)
    .filter(([, entry]) => entry && typeof entry.until === "number" && entry.until > now)
    .sort(([, a], [, b]) => b.until - a.until)
    .slice(0, MAX_ENTRIES);
  try {
    localStorage.setItem(DISMISS_KEY, JSON.stringify(Object.fromEntries(live)));
  } catch {
    /* private mode */
  }
}

/** When a dismissal made now should lapse. */
export function dismissUntil(resetsAt: string | null, now: number): number {
  const at = resetsAt ? Date.parse(resetsAt) : Number.NaN;
  return Number.isFinite(at) && at > now ? at : now + DEFAULT_DISMISS_MS;
}

export function dismiss(
  sessionKey: string,
  level: Exclude<LimitLevel, "ok">,
  resetsAt: string | null,
  now = Date.now(),
): void {
  const table = read();
  table[sessionKey] = { level, until: dismissUntil(resetsAt, now) };
  write(table, now);
}

export function isDismissed(sessionKey: string, level: LimitLevel, now = Date.now()): boolean {
  if (level === "ok") return false;
  const entry = read()[sessionKey];
  if (!entry || entry.until <= now) return false;
  return entry.level === "out" || entry.level === level;
}
