/**
 * Is the runner this session talks to about to stop, or already stopped?
 *
 * Two sources, both read here and nowhere else:
 * - the usage report — a window of the runner's account at or under the user's warning
 *   threshold (5% unless changed in settings) is "near", an empty one is "out";
 * - the transcript — a turn that failed with a provider limit is "out". The backend tags
 *   those errors with a structured `limit` (see `src/usage/limit_error.rs`), so this file
 *   never reads provider prose. A transcript without tags simply reports nothing.
 *
 * Pure, so the thresholds are testable without a DOM or a clock.
 */
import type { Message } from "../types";
import type { UsageAccount, UsageReport, UsageWindow } from "../usage/usageClient";
import { accountForRunner, remainingPercent } from "../usage/format";
import { parseHandoffBlock, parseMemoryBlock } from "../message-turn/helpers";

export type LimitLevel = "ok" | "near" | "out";
export type LimitKind = "usage" | "rate";

/** At or under this much left, a window is close enough to act on. Settings can change it. */
export const NEAR_PERCENT = 5;
/** A failure without a reset time stops counting after this long. */
export const FAILURE_TTL_MS = 60 * 60_000;

export interface LimitTag {
  readonly kind: LimitKind;
  readonly resetsAt: string | null;
}

/** A turn the provider refused because the account ran out. */
export interface FailedTurn {
  readonly messageId: string;
  readonly kind: LimitKind;
  readonly resetsAt: string | null;
  /** When the failure was reported, client clock. */
  readonly at: number;
  /** The prompt that failed, as the user typed it. Null when it cannot be recovered. */
  readonly prompt: string | null;
}

export interface RunnerLimit {
  readonly level: LimitLevel;
  readonly account: UsageAccount | null;
  /** The window that triggered the level, when it came from the report. */
  readonly window: UsageWindow | null;
  readonly leftPercent: number | null;
  readonly resetsAt: string | null;
  readonly failed: FailedTurn | null;
}

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

/** The backend's `limit` tag on a message, or null. Never throws on odd shapes. */
export function readLimitTag(info: unknown): LimitTag | null {
  if (!isObject(info) || !isObject(info.limit)) return null;
  const { kind, resetsAt } = info.limit;
  if (kind !== "usage" && kind !== "rate") return null;
  return { kind, resetsAt: typeof resetsAt === "string" && resetsAt ? resetsAt : null };
}

function timeOf(message: Message): number {
  const time = message.info.time;
  if (typeof time === "number") return time;
  return time?.created ?? 0;
}

function idOf(message: Message): string {
  return message.info.id ?? message.info.messageID ?? "";
}

/** What the user typed: a handoff's transcript and a session-instructions block removed. */
export function userPromptText(message: Message): string | null {
  const raw = message.parts
    .filter((part) => part.type === "text" && typeof part.text === "string")
    .map((part) => part.text as string)
    .join("\n")
    .trim();
  if (!raw) return null;
  const handoff = parseHandoffBlock(raw);
  const afterHandoff = handoff ? handoff.userText : raw;
  const memory = parseMemoryBlock(afterHandoff);
  const text = (memory ? memory.userText : afterHandoff).trim();
  return text || null;
}

/**
 * The limit failure that ended the latest turn, if it did. A user message after the
 * failure means the conversation moved on, so it no longer counts.
 */
export function lastTurnLimit(messages: readonly Message[]): FailedTurn | null {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.info.role === "user") return null;
    const tag = readLimitTag(message.info);
    if (!tag) continue;
    let prompt: string | null = null;
    for (let j = i - 1; j >= 0; j -= 1) {
      if (messages[j].info.role === "user") {
        prompt = userPromptText(messages[j]);
        break;
      }
    }
    return { messageId: idOf(message), kind: tag.kind, resetsAt: tag.resetsAt, at: timeOf(message), prompt };
  }
  return null;
}

function parseTime(iso: string | null): number | null {
  if (!iso) return null;
  const at = Date.parse(iso);
  return Number.isNaN(at) ? null : at;
}

/** A window whose reset time has passed is a reading of a window that no longer exists. */
function isLive(window: UsageWindow, now: number): boolean {
  const reset = parseTime(window.resetsAt);
  return reset === null || reset > now;
}

/** Whether a failure still describes the account: not yet reset, and not ancient. */
export function failureStillApplies(failed: FailedTurn, now: number): boolean {
  const reset = parseTime(failed.resetsAt);
  if (reset !== null) return reset > now;
  // A rate limit is transient; a spent plan without a reset time gets the TTL too.
  return failed.at <= 0 || now - failed.at < FAILURE_TTL_MS;
}

/** The live window with the least left, from an account whose reading is current. */
export function tightestWindow(account: UsageAccount | null, now: number): UsageWindow | null {
  if (!account || account.state !== "ok") return null;
  const live = account.windows.filter((window) => isLive(window, now));
  if (live.length === 0) return null;
  return live.reduce((tightest, window) =>
    remainingPercent(window) < remainingPercent(tightest) ? window : tightest);
}

/**
 * Only the percentage counts, not the server's severity: the server calls a tenth left
 * "critical", and a user who asked to be told at 5% should not hear about it at 10%.
 */
export function levelOfWindow(window: UsageWindow | null, nearPercent = NEAR_PERCENT): LimitLevel {
  if (!window) return "ok";
  const left = remainingPercent(window);
  if (left <= 0) return "out";
  if (left <= nearPercent) return "near";
  return "ok";
}

export function runnerLimit(
  report: UsageReport | null,
  runner: string,
  failed: FailedTurn | null,
  now: number,
  nearPercent = NEAR_PERCENT,
): RunnerLimit {
  const account = accountForRunner(report, runner);
  const window = tightestWindow(account, now);
  const fromWindow = levelOfWindow(window, nearPercent);
  const failure = failed && failureStillApplies(failed, now) ? failed : null;
  const level: LimitLevel = failure ? "out" : fromWindow;
  return {
    level,
    account,
    window: fromWindow === "ok" ? null : window,
    leftPercent: window ? remainingPercent(window) : null,
    resetsAt: failure?.resetsAt ?? (fromWindow === "ok" ? null : window?.resetsAt ?? null),
    failed: failure,
  };
}
