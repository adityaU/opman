/**
 * Everything a composer needs to know about its runner running out.
 *
 * Reads the shared usage report (re-read every minute while the page is visible, by
 * `useUsage`), the transcript's last limit failure, the user's policy, threshold and
 * fallback list, and the per-session "not now". Recomputed against a clock that ticks
 * every 30 seconds, so a window that resets, or a dismissal that lapses, is noticed
 * without waiting for the next report.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import type { Message } from "../types";
import { useUsage } from "../usage/useUsage";
import { limitOptions, type LimitTarget } from "./chooseTarget";
import { dismiss as storeDismissal, isDismissed } from "./dismissals";
import { useFallbacks } from "./fallbacks";
import { useLimitPolicy, type LimitPolicy } from "./limitPolicy";
import { lastTurnLimit, runnerLimit, type RunnerLimit } from "./limitState";

const TICK_MS = 30_000;
const NO_MESSAGES: readonly Message[] = [];

export interface RunnerLimitView {
  readonly limit: RunnerLimit;
  readonly options: readonly LimitTarget[];
  readonly policy: LimitPolicy;
  /** The user said "not now" to this level, or already switched away from it. */
  readonly dismissed: boolean;
  /** Key the dismissal is stored under. */
  readonly key: string;
  readonly dismiss: () => void;
}

function useNow(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), TICK_MS);
    return () => window.clearInterval(timer);
  }, []);
  return now;
}

export function useRunnerLimit(
  sessionId: string | null,
  runner: string,
  available: readonly string[],
  messages: readonly Message[] = NO_MESSAGES,
): RunnerLimitView {
  const usage = useUsage();
  const [policy] = useLimitPolicy();
  const { nearPercent, pairs } = useFallbacks();
  const now = useNow();
  // Bumped by a dismissal so the stored record is re-read without waiting for the clock.
  const [revision, setRevision] = useState(0);

  const failed = useMemo(() => lastTurnLimit(messages), [messages]);
  const limit = useMemo(
    () => runnerLimit(usage.report, runner, failed, now, nearPercent),
    [usage.report, runner, failed, now, nearPercent],
  );
  const options = useMemo(
    () => limitOptions({
      report: usage.report, current: runner, level: limit.level, pairs, available, now, nearPercent,
    }),
    [usage.report, runner, limit.level, pairs, available, now, nearPercent],
  );

  // Scoped to the runner as well as the session: "not now" about Claude says nothing
  // about Codex running out later in the same conversation.
  const key = `${sessionId ?? "new"}:${runner}`;
  const dismissed = useMemo(
    () => isDismissed(key, limit.level, now),
    // `revision` re-reads storage after a dismissal made in this render cycle.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [key, limit.level, now, revision],
  );

  const dismiss = useCallback(
    () => {
      if (limit.level === "ok") return;
      storeDismissal(key, limit.level, limit.resetsAt, Date.now());
      setRevision((value) => value + 1);
    },
    [key, limit.level, limit.resetsAt],
  );
  return { limit, options, policy, dismissed, key, dismiss };
}
