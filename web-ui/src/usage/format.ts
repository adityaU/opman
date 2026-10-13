/**
 * Turning a usage report into what a person reads: how much is left, when it comes back,
 * and how worried to be. Pure, so the picker's copy is testable without a DOM.
 *
 * The backend speaks in *used*; the picker speaks in *left*, because the question being
 * answered at the moment of choosing a runner is "can this one still take the turn?".
 */
import type { UsageAccount, UsageReport, UsageSeverity, UsageWindow } from "./usageClient";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Whole percent remaining in a window, 0–100. */
export function remainingPercent(window: Pick<UsageWindow, "usedPercent">): number {
  const left = 100 - window.usedPercent;
  if (!Number.isFinite(left)) return 0;
  return Math.round(Math.min(100, Math.max(0, left)));
}

/**
 * The server's severity when it sent one, else derived from what is left. The thresholds
 * mirror how the CLIs themselves start warning: a quarter left, then a tenth.
 */
export function severityOf(window: UsageWindow): UsageSeverity {
  if (window.severity) return window.severity;
  const left = remainingPercent(window);
  if (left <= 10) return "critical";
  if (left <= 25) return "warning";
  return "normal";
}

/** "1h 52m", "3d 4h", "42m", "<1m" — the duration alone. Null when unknown. */
export function formatDuration(ms: number): string {
  if (ms < MINUTE) return "<1m";
  if (ms < HOUR) return `${Math.floor(ms / MINUTE)}m`;
  if (ms < DAY) {
    const hours = Math.floor(ms / HOUR);
    const minutes = Math.floor((ms % HOUR) / MINUTE);
    return minutes ? `${hours}h ${minutes}m` : `${hours}h`;
  }
  const days = Math.floor(ms / DAY);
  const hours = Math.floor((ms % DAY) / HOUR);
  return hours ? `${days}d ${hours}h` : `${days}d`;
}

function parseTime(iso: string | null | undefined): number | null {
  if (!iso) return null;
  const at = Date.parse(iso);
  return Number.isNaN(at) ? null : at;
}

/** "resets in 1h 52m" / "resets now", or null when the reset time is unknown. */
export function formatResetIn(resetsAt: string | null, now = Date.now()): string | null {
  const at = parseTime(resetsAt);
  if (at === null) return null;
  const ms = at - now;
  return ms <= 0 ? "resets now" : `resets in ${formatDuration(ms)}`;
}

/** "just now" / "2m ago" / "3h ago" for a reading's age. */
export function formatAge(iso: string | null, now = Date.now()): string | null {
  const at = parseTime(iso);
  if (at === null) return null;
  const ms = now - at;
  return ms < MINUTE ? "just now" : `${formatDuration(ms)} ago`;
}

/** The window with the least left — the one that will stop the runner first. */
export function mostConstrained(account: UsageAccount | null): UsageWindow | null {
  if (!account || account.windows.length === 0) return null;
  return account.windows.reduce((tightest, window) =>
    remainingPercent(window) < remainingPercent(tightest) ? window : tightest);
}

export function accountForRunner(report: UsageReport | null, runner: string): UsageAccount | null {
  if (!report) return null;
  const id = report.runners[runner];
  if (!id) return null;
  return report.accounts.find((account) => account.id === id) ?? null;
}

/**
 * What to say instead of (or beside) the meters. The server's own note wins — it knows
 * the specific reason — then a default per state. Null for a healthy account.
 */
export function stateNote(account: UsageAccount): string | null {
  if (account.note) return account.note;
  switch (account.state) {
    case "expired": return `Open \`${account.id}\` once to refresh its login`;
    case "signed_out": return `Signed out — run \`${account.id}\` to sign in`;
    case "api_key": return "API key — pay as you go";
    case "error": return "Couldn’t read usage";
    case "unsupported": return "Usage not reported";
    default: return null;
  }
}

/** The tone a note is written in: something to fix, or just a fact. */
export function noteTone(account: UsageAccount): "warning" | "muted" {
  return account.state === "expired" || account.state === "signed_out" || account.state === "error"
    ? "warning"
    : "muted";
}

/** One sentence for assistive tech: the account at a glance. */
export function describeUsage(account: UsageAccount | null, now = Date.now()): string {
  if (!account) return "Usage not reported";
  const parts = account.windows.map((window) => {
    const reset = formatResetIn(window.resetsAt, now);
    return `${window.label}: ${remainingPercent(window)}% left${reset ? `, ${reset}` : ""}`;
  });
  const note = stateNote(account);
  if (note) parts.unshift(note.replace(/`/g, ""));
  if (account.stale) parts.push("last reading may be out of date");
  return parts.join(". ") || "Usage not reported";
}

/** A plan id as a tag: `pro` → `Pro`, `max_5x` → `Max 5x`. */
export function planLabel(plan: string): string {
  const words = plan.replace(/[_-]+/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}
