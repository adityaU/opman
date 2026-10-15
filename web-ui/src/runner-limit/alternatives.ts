/**
 * Which other runner could take this conversation over.
 *
 * Only runners that would actually get further qualify:
 * - never one billed to the same account — Claude over ACP and Claude Code share one
 *   allowance, so "switch to the other Claude" just fails the same way;
 * - a runner whose account is known to have room comes first, most headroom (in its
 *   tightest window) first;
 * - a runner that reports no usage at all (opencode, most ACP agents, an API key) comes
 *   after those: it may well work, but nothing says so;
 * - a signed-out or unreadable account is not offered at all.
 *
 * An empty list is the signal to show nothing: with one runner, or one account, there is
 * nowhere to go.
 */
import type { UsageAccount, UsageReport } from "../usage/usageClient";
import { remainingPercent } from "../usage/format";
import { levelOfWindow, NEAR_PERCENT, tightestWindow, type LimitLevel } from "./limitState";

export interface Alternative {
  readonly runner: string;
  readonly account: UsageAccount | null;
  /** Percent left in the tightest window; null when the runner reports no usage. */
  readonly leftPercent: number | null;
}

/** Accounts that cannot take a turn at all right now. */
const UNUSABLE = new Set(["signed_out", "error"]);

function accountOf(report: UsageReport | null, runner: string): UsageAccount | null {
  const id = report?.runners[runner];
  if (!id) return null;
  return report?.accounts.find((account) => account.id === id) ?? null;
}

interface Scored extends Alternative {
  readonly tier: number;
}

/**
 * Rank the runners the session could move to. `currentLevel` decides how picky to be: a
 * runner that is itself near its limit is only worth offering when the current one is out.
 */
export function rankAlternatives(
  report: UsageReport | null,
  current: string,
  runners: readonly string[],
  now: number,
  currentLevel: LimitLevel = "out",
  nearPercent = NEAR_PERCENT,
): Alternative[] {
  const currentAccount = report?.runners[current] ?? null;
  const scored: Scored[] = [];
  for (const runner of new Set(runners)) {
    if (runner === current) continue;
    const accountId = report?.runners[runner] ?? null;
    if (currentAccount && accountId === currentAccount) continue;
    const account = accountOf(report, runner);
    if (account && UNUSABLE.has(account.state)) continue;
    const window = tightestWindow(account, now);
    if (!window) {
      // Known account without a usable reading, or a runner that reports nothing.
      scored.push({ runner, account, leftPercent: null, tier: 2 });
      continue;
    }
    const level = levelOfWindow(window, nearPercent);
    if (level === "out") continue;
    if (level === "near" && currentLevel !== "out") continue;
    scored.push({ runner, account, leftPercent: remainingPercent(window), tier: level === "ok" ? 0 : 1 });
  }
  scored.sort((a, b) => a.tier - b.tier || (b.leftPercent ?? 0) - (a.leftPercent ?? 0));
  return scored.map(({ runner, account, leftPercent }) => ({ runner, account, leftPercent }));
}
