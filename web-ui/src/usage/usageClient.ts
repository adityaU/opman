/**
 * The `/api/usage` contract and a client that never throws.
 *
 * Usage is decoration on top of choosing an engine, never a precondition for it, so every
 * way the request can go wrong collapses into one of three answers the UI can render:
 * a report, "this server does not report usage" (an older backend: a 404, or the SPA
 * fallback answering with `index.html`), or a transient error worth a retry.
 *
 * The body is normalised field by field rather than trusted. The backend reads these
 * numbers out of other tools' private APIs, and one malformed window must cost that window,
 * not the whole picker.
 */
import { apiUrl } from "../api/base";

export type UsageState = "ok" | "expired" | "signed_out" | "api_key" | "error" | "unsupported";
export type UsageSeverity = "normal" | "warning" | "critical";

export interface UsageWindow {
  id: string;
  label: string;
  /** 0–100, already clamped. */
  usedPercent: number;
  resetsAt: string | null;
  severity: UsageSeverity | null;
}

export interface UsageCredits {
  balance: string | null;
  enabled: boolean;
}

export interface UsageAccount {
  id: string;
  label: string;
  state: UsageState;
  plan: string | null;
  windows: UsageWindow[];
  credits: UsageCredits | null;
  note: string | null;
  stale: boolean;
  fetchedAt: string | null;
}

export interface UsageReport {
  accounts: UsageAccount[];
  /** runner id → account id. A runner missing here does not report usage. */
  runners: Record<string, string>;
}

export type UsageFetchResult =
  | { kind: "ok"; report: UsageReport }
  | { kind: "unavailable" }
  | { kind: "error"; message: string };

const STATES: readonly UsageState[] = ["ok", "expired", "signed_out", "api_key", "error", "unsupported"];
const SEVERITIES: readonly UsageSeverity[] = ["normal", "warning", "critical"];
/** Statuses that mean "this server has no such route", not "it failed". */
const ABSENT = new Set([404, 405, 501]);

type Json = Record<string, unknown>;

const isObject = (value: unknown): value is Json =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const str = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value : null;

function parseWindow(raw: unknown): UsageWindow | null {
  if (!isObject(raw)) return null;
  const used = typeof raw.usedPercent === "number" ? raw.usedPercent : Number.NaN;
  const id = str(raw.id);
  if (!id || !Number.isFinite(used)) return null;
  const severity = SEVERITIES.includes(raw.severity as UsageSeverity) ? (raw.severity as UsageSeverity) : null;
  return {
    id,
    label: str(raw.label) ?? id,
    usedPercent: Math.min(100, Math.max(0, used)),
    resetsAt: str(raw.resetsAt),
    severity,
  };
}

function parseAccount(raw: unknown): UsageAccount | null {
  if (!isObject(raw)) return null;
  const id = str(raw.id);
  if (!id) return null;
  const credits = isObject(raw.credits)
    ? { balance: str(raw.credits.balance), enabled: raw.credits.enabled === true }
    : null;
  return {
    id,
    label: str(raw.label) ?? id,
    // A state this client does not know yet is shown as "not reported" rather than as an
    // alarm: an unknown word is no evidence that anything is wrong.
    state: STATES.includes(raw.state as UsageState) ? (raw.state as UsageState) : "unsupported",
    plan: str(raw.plan),
    windows: Array.isArray(raw.windows)
      ? raw.windows.map(parseWindow).filter((w): w is UsageWindow => w !== null)
      : [],
    credits,
    note: str(raw.note),
    stale: raw.stale === true,
    fetchedAt: str(raw.fetchedAt),
  };
}

/** Normalise an untrusted body. Null when it is not a usage report at all. */
export function parseUsageReport(body: unknown): UsageReport | null {
  if (!isObject(body) || !Array.isArray(body.accounts)) return null;
  const runners: Record<string, string> = {};
  if (isObject(body.runners)) {
    for (const [runner, account] of Object.entries(body.runners)) {
      if (typeof account === "string" && account) runners[runner] = account;
    }
  }
  return {
    accounts: body.accounts.map(parseAccount).filter((a): a is UsageAccount => a !== null),
    runners,
  };
}

async function errorMessage(res: Response): Promise<string> {
  try {
    const body: unknown = await res.json();
    if (isObject(body)) {
      const message = str(body.error) ?? str(body.message);
      if (message) return message;
    }
  } catch {
    /* fall through to the status line */
  }
  return `Usage request failed (${res.status})`;
}

/** GET /api/usage. `refresh` asks the server to re-read rather than serve its cache. */
export async function fetchUsage(options: { refresh?: boolean; signal?: AbortSignal } = {}): Promise<UsageFetchResult> {
  let res: Response;
  try {
    res = await fetch(apiUrl(options.refresh ? "/usage?refresh=1" : "/usage"), {
      credentials: "same-origin",
      headers: { Accept: "application/json" },
      signal: options.signal,
    });
  } catch (error: unknown) {
    return { kind: "error", message: error instanceof Error ? error.message : "Network error" };
  }
  if (ABSENT.has(res.status)) return { kind: "unavailable" };
  if (!res.ok) return { kind: "error", message: await errorMessage(res) };
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    // A 200 that is not JSON is the SPA fallback: a backend that predates the route.
    return { kind: "unavailable" };
  }
  const report = parseUsageReport(body);
  return report ? { kind: "ok", report } : { kind: "unavailable" };
}
